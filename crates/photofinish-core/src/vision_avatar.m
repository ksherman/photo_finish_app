#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ImageIO/ImageIO.h>
#import <Vision/Vision.h>
#include <math.h>
#include <stddef.h>

static CGRect pf_vision_to_top_down(CGRect r, CGFloat imgW, CGFloat imgH) {
    CGFloat x = r.origin.x * imgW;
    CGFloat y = (1.0 - r.origin.y - r.size.height) * imgH;
    CGFloat w = r.size.width * imgW;
    CGFloat h = r.size.height * imgH;
    return CGRectMake(x, y, w, h);
}

static CGRect pf_clamp_square_crop(CGFloat cx, CGFloat cy, CGFloat side, CGFloat imgW, CGFloat imgH) {
    CGFloat maxSide = fmin(imgW, imgH);
    CGFloat minSide = fmin(140.0, maxSide);
    CGFloat s = fmin(fmax(side, minSide), maxSide);
    CGFloat x = cx - s / 2.0;
    CGFloat y = cy - s / 2.0;
    if (x < 0.0) x = 0.0;
    if (y < 0.0) y = 0.0;
    if (x + s > imgW) x = imgW - s;
    if (y + s > imgH) y = imgH - s;
    return CGRectMake(round(x), round(y), round(s), round(s));
}

static CGImageRef pf_load_cg_image(NSString *path) {
    NSURL *url = [NSURL fileURLWithPath:path];
    if (!url) return NULL;
    CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)url, NULL);
    if (!src) return NULL;
    CGImageRef img = CGImageSourceCreateImageAtIndex(src, 0, NULL);
    CFRelease(src);
    return img;
}

static BOOL pf_write_square_avatar(CGImageRef srcImg, CGRect cropRect, NSString *outPath, int outSize, double quality) {
    if (!srcImg || outSize <= 0) return NO;
    CGImageRef cropped = CGImageCreateWithImageInRect(srcImg, cropRect);
    if (!cropped) return NO;

    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    if (!cs) {
        CGImageRelease(cropped);
        return NO;
    }

    CGContextRef ctx = CGBitmapContextCreate(
        NULL,
        (size_t)outSize,
        (size_t)outSize,
        8,
        (size_t)outSize * 4,
        cs,
        kCGImageAlphaNoneSkipLast | kCGBitmapByteOrderDefault
    );
    CGColorSpaceRelease(cs);

    if (!ctx) {
        CGImageRelease(cropped);
        return NO;
    }

    CGContextSetInterpolationQuality(ctx, kCGInterpolationHigh);
    CGContextDrawImage(ctx, CGRectMake(0, 0, outSize, outSize), cropped);
    CGImageRef resized = CGBitmapContextCreateImage(ctx);
    CGContextRelease(ctx);
    CGImageRelease(cropped);

    if (!resized) return NO;

    NSString *parent = [outPath stringByDeletingLastPathComponent];
    [[NSFileManager defaultManager] createDirectoryAtPath:parent
                              withIntermediateDirectories:YES
                                               attributes:nil
                                                    error:nil];

    NSString *tmpPath = [NSString stringWithFormat:@"%@.tmp.%u", outPath, arc4random()];
    NSURL *tmpUrl = [NSURL fileURLWithPath:tmpPath];
    CGImageDestinationRef dest = CGImageDestinationCreateWithURL(
        (__bridge CFURLRef)tmpUrl,
        CFSTR("public.jpeg"),
        1,
        NULL
    );

    BOOL ok = NO;
    if (dest) {
        NSDictionary *props = @{
            (__bridge NSString *)kCGImageDestinationLossyCompressionQuality: @(quality)
        };
        CGImageDestinationAddImage(dest, resized, (__bridge CFDictionaryRef)props);
        ok = CGImageDestinationFinalize(dest);
        CFRelease(dest);
    }
    CGImageRelease(resized);

    if (!ok) {
        [[NSFileManager defaultManager] removeItemAtPath:tmpPath error:nil];
        return NO;
    }

    NSURL *finalUrl = [NSURL fileURLWithPath:outPath];
    NSError *moveErr = nil;
    [[NSFileManager defaultManager] removeItemAtURL:finalUrl error:nil];
    if (![[NSFileManager defaultManager] moveItemAtURL:tmpUrl toURL:finalUrl error:&moveErr]) {
        [[NSFileManager defaultManager] removeItemAtPath:tmpPath error:nil];
        return NO;
    }
    return YES;
}

static BOOL pf_point_in_salient(NSArray<NSValue *> *salientBoxes, CGPoint pt) {
    for (NSValue *v in salientBoxes) {
        CGRect sb;
        [v getValue:&sb];
        if (CGRectContainsPoint(CGRectInset(sb, -0.05, -0.05), pt)) {
            return YES;
        }
    }
    return NO;
}

int pf_vision_is_available(void) {
    return 1;
}

/// Evaluates up to `path_count` preview images using Apple's Neural Engine Vision framework
/// (VNDetectFaceCaptureQualityRequest + VNGenerateAttentionBasedSaliencyImageRequest), selects
/// the best solo foreground gymnast face (or attention-saliency fallback), crops a generous
/// head-and-shoulders + leotard square, and saves a `output_size_px` JPEG to `output_jpeg_path`.
///
/// Returns:
///   1 = Face-detected avatar written
///   2 = Attention-saliency fallback avatar written
///   0 = No readable images / no region detected
///  -1 = Error writing output
int pf_vision_generate_avatar(
    const char *const *preview_paths,
    size_t path_count,
    const char *output_jpeg_path,
    int output_size_px,
    double jpeg_quality
) {
    if (!preview_paths || path_count == 0 || !output_jpeg_path || output_size_px <= 0) {
        return 0;
    }

    @autoreleasepool {
        NSString *outPath = [NSString stringWithUTF8String:output_jpeg_path];
        if (!outPath) return -1;

        BOOL hasBestFace = NO;
        double bestFaceScore = -1.0;
        float bestFaceQuality = 0.0f;
        BOOL bestFaceInSal = NO;
        NSString *bestFacePath = nil;
        CGRect bestFaceCrop = CGRectZero;

        BOOL hasBestFallback = NO;
        double bestFallbackScore = -1.0;
        NSString *bestFallbackPath = nil;
        CGRect bestFallbackCrop = CGRectZero;

        for (size_t i = 0; i < path_count; i++) {
            if (!preview_paths[i]) continue;
            NSString *imgPath = [NSString stringWithUTF8String:preview_paths[i]];
            if (!imgPath) continue;

            CGImageRef cg = pf_load_cg_image(imgPath);
            if (!cg) continue;

            CGFloat imgW = (CGFloat)CGImageGetWidth(cg);
            CGFloat imgH = (CGFloat)CGImageGetHeight(cg);
            if (imgW < 32.0 || imgH < 32.0) {
                CGImageRelease(cg);
                continue;
            }

            VNDetectFaceCaptureQualityRequest *faceReq = [[VNDetectFaceCaptureQualityRequest alloc] init];
            VNGenerateAttentionBasedSaliencyImageRequest *salReq = [[VNGenerateAttentionBasedSaliencyImageRequest alloc] init];
            VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithCGImage:cg options:@{}];

            NSError *reqErr = nil;
            [handler performRequests:@[faceReq, salReq] error:&reqErr];

            // Extract foreground salient bounding boxes (Vision normalized coords, bottom-left origin)
            NSMutableArray<NSValue *> *salientBoxes = [NSMutableArray array];
            BOOL hasUnionSal = NO;
            CGRect unionSal = CGRectZero;

            VNSaliencyImageObservation *salObs = salReq.results.firstObject;
            if (salObs && salObs.salientObjects.count > 0) {
                for (VNRectangleObservation *obj in salObs.salientObjects) {
                    CGRect sb = obj.boundingBox;
                    [salientBoxes addObject:[NSValue valueWithBytes:&sb objCType:@encode(CGRect)]];
                    unionSal = hasUnionSal ? CGRectUnion(unionSal, sb) : sb;
                    hasUnionSal = YES;
                }
            }

            NSArray<VNFaceObservation *> *faces = faceReq.results;
            if (faces.count > 0) {
                // Count prominent foreground faces so solo performance frames strongly beat
                // end-of-routine coach hugs / teammate high-fives.
                NSUInteger prominentCount = 0;
                for (VNFaceObservation *f in faces) {
                    CGRect bb = f.boundingBox;
                    float q = f.faceCaptureQuality ? f.faceCaptureQuality.floatValue : 0.0f;
                    BOOL bigEnough = (bb.size.width >= 0.03 || bb.size.height >= 0.03);
                    CGPoint fc = CGPointMake(CGRectGetMidX(bb), CGRectGetMidY(bb));
                    BOOL inSal = pf_point_in_salient(salientBoxes, fc);
                    if (bigEnough && (inSal || q >= 0.25f)) {
                        prominentCount += 1;
                    }
                }
                double soloBonus = (prominentCount == 1) ? 2.2 : ((prominentCount == 2) ? 0.55 : 0.35);

                for (VNFaceObservation *face in faces) {
                    CGRect bb = face.boundingBox;
                    if (bb.size.width < 0.025 && bb.size.height < 0.025) continue;

                    float q = face.faceCaptureQuality ? face.faceCaptureQuality.floatValue : 0.0f;
                    double area = (double)(bb.size.width * bb.size.height);
                    CGPoint fc = CGPointMake(CGRectGetMidX(bb), CGRectGetMidY(bb));
                    BOOL inSal = pf_point_in_salient(salientBoxes, fc);

                    double dx = fabs((double)CGRectGetMidX(bb) - 0.5);
                    double dy = fabs((double)CGRectGetMidY(bb) - 0.58);
                    double centerWeight = fmax(0.25, 1.0 - (dx * 0.9 + dy * 0.5));

                    double rollRad = face.roll ? fabs(face.roll.doubleValue) : 0.0;
                    double uprightBonus = (rollRad < 0.45) ? 1.25 : ((rollRad < 0.9) ? 0.85 : 0.35);
                    double saliencyBonus = inSal ? 2.5 : 0.35;

                    double qClamped = fmax((double)q, 0.05);
                    double score = pow(qClamped, 1.4) * pow(area, 0.45) * centerWeight * uprightBonus * saliencyBonus * soloBonus;

                    if (!hasBestFace || score > bestFaceScore) {
                        CGRect facePx = pf_vision_to_top_down(bb, imgW, imgH);
                        CGFloat cropSide = fmax(facePx.size.width, facePx.size.height) * 4.2;
                        CGFloat cropCX = CGRectGetMidX(facePx);
                        CGFloat cropCY = CGRectGetMidY(facePx) + facePx.size.height * 0.65;
                        bestFaceCrop = pf_clamp_square_crop(cropCX, cropCY, cropSide, imgW, imgH);
                        bestFaceScore = score;
                        bestFaceQuality = q;
                        bestFaceInSal = inSal;
                        bestFacePath = imgPath;
                        hasBestFace = YES;
                    }
                }
            }

            if (hasUnionSal) {
                double salArea = (double)(unionSal.size.width * unionSal.size.height);
                if (!hasBestFallback || salArea > bestFallbackScore) {
                    CGRect sPx = pf_vision_to_top_down(unionSal, imgW, imgH);
                    CGFloat side = fmax(sPx.size.width, sPx.size.height) * 1.15;
                    CGFloat cy = sPx.origin.y + fmin(side * 0.42, sPx.size.height * 0.45);
                    bestFallbackCrop = pf_clamp_square_crop(CGRectGetMidX(sPx), cy, side, imgW, imgH);
                    bestFallbackScore = salArea;
                    bestFallbackPath = imgPath;
                    hasBestFallback = YES;
                }
            }

            CGImageRelease(cg);
        }

        NSString *chosenPath = nil;
        CGRect chosenCrop = CGRectZero;
        int methodCode = 0;

        if (hasBestFace && (bestFaceQuality >= 0.18f || bestFaceInSal)) {
            chosenPath = bestFacePath;
            chosenCrop = bestFaceCrop;
            methodCode = 1;
        } else if (hasBestFallback) {
            chosenPath = bestFallbackPath;
            chosenCrop = bestFallbackCrop;
            methodCode = 2;
        } else if (hasBestFace) {
            chosenPath = bestFacePath;
            chosenCrop = bestFaceCrop;
            methodCode = 1;
        }

        if (!chosenPath) return 0;

        CGImageRef winningImg = pf_load_cg_image(chosenPath);
        if (!winningImg) return -1;

        BOOL wrote = pf_write_square_avatar(winningImg, chosenCrop, outPath, output_size_px, jpeg_quality);
        CGImageRelease(winningImg);
        return wrote ? methodCode : -1;
    }
}
