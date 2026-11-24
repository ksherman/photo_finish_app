use super::*;
use crate::orders::CreateOrderItemInput;
use crate::roster::{FolderReassignmentInput, RemoveFolderMode, RosterEntryInput};
use tempfile::tempdir;

fn test_bundle(root: PathBuf, name: &str, order_code: &str) -> EventBundle {
    EventBundle::create(CreateEventParams {
        root_path: root,
        name: name.into(),
        order_code: Some(order_code.into()),
        ..Default::default()
    })
    .unwrap()
}

fn write_test_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn creates_reopens_and_gathers_multiple_event_bundles() {
    let season_dir = tempdir().unwrap();
    let meet_a_dir = season_dir.path().join("2026-03-13_IL-Level-3");
    let meet_b_dir = season_dir.path().join("2026-03-13_IN-Level-4");

    let bundle_a = EventBundle::create(CreateEventParams {
        root_path: meet_a_dir.clone(),
        name: "IL Level 3 State".into(),
        order_code: Some("IL3".into()),
        station_code: Some("A".into()),
        tax_rate_basis_points: Some(850),
        scaffold: Some(ScaffoldConfig {
            num_areas: Some(1),
            area_prefix: Some("Gym".into()),
            sessions_per_area: Some(2),
            session_prefix: Some("Session".into()),
        }),
        ..Default::default()
    })
    .unwrap();

    assert!(meet_a_dir.join("_photofinish/photofinish.db").is_file());
    assert!(meet_a_dir.join("Gym A/Session 1A").is_dir());

    let sub = bundle_a
        .add_subject(Some("1059"), "Iza", Some("Z"), Some("1A"), Some("Flippers"))
        .unwrap();
    assert_eq!(sub.display_name, "1059 Iza Z");

    let (_batch_id, jobs) = bundle_a
        .record_ingest_batch(
            IngestBatchMeta {
                reader_id: Some("USB-SERIAL-1".into()),
                reader_name: Some("External USB Reader".into()),
                volume_name: Some("EOS_DIGITAL".into()),
                photographer: Some("KDS".into()),
                source_path: Some("/Volumes/EOS_DIGITAL/DCIM".into()),
                destination_rel_path: "Gym A/Session 1A/Group 2B/Beam".into(),
                tags: BTreeMap::new(),
            },
            &[IngestFileInput {
                relative_path: "Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg".into(),
                original_filename: "IMG_001.jpg".into(),
                file_size_bytes: 4096,
            }],
        )
        .unwrap();
    assert_eq!(jobs.len(), 1);
    bundle_a
        .mark_photo_ready(
            &jobs[0].photo_id,
            "_photofinish/thumbnails/test.jpg",
            "_photofinish/previews/test.jpg",
            1200,
            800,
        )
        .unwrap();

    let matches = bundle_a.search_subjects("1059", 10).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].photo_count, 1);

    EventBundle::create(CreateEventParams {
        root_path: meet_b_dir,
        name: "IN Level 4 State".into(),
        preset_id: Some("dance".into()),
        order_code: Some("IN4".into()),
        station_code: Some("B".into()),
        ..Default::default()
    })
    .unwrap();

    let gathered = EventBundle::discover_in_directory(season_dir.path()).unwrap();
    assert_eq!(gathered.len(), 2);
    assert_eq!(gathered[0].name, "IL Level 3 State");
    assert_eq!(gathered[0].photo_counts.ready, 1);
    assert_eq!(gathered[1].name, "IN Level 4 State");
    assert_eq!(gathered[1].subject_label, "Dancer");
}

#[test]
fn imports_roster_auto_matches_and_assigns_folders_with_safe_merge() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("xcel-test");
    let bundle = test_bundle(root.clone(), "Xcel Test", "XCL");

    for rel in [
        "Gym A/Session 1A/Group 1A/Beam/105 Julia B/IMG_1.jpg",
        "Gym A/Session 1A/Group 1A/Beam/Gymnast 05/IMG_2.jpg",
        "Gym A/Session 1A/Group 1A/Beam/Gymnast 06/IMG_3.jpg",
    ] {
        write_test_file(&root.join(rel), b"fakejpg");
    }
    assert_eq!(bundle.scan_directory().unwrap().photos_new, 3);

    let entries = vec![
        RosterEntryInput {
            number: Some("105".into()),
            first_name: "Julia".into(),
            last_name: Some("B".into()),
            group_label: Some("Session 1A".into()),
            team_name: None,
            external_id: None,
        },
        RosterEntryInput {
            number: Some("143".into()),
            first_name: "Avery".into(),
            last_name: Some("W".into()),
            group_label: Some("Session 1A".into()),
            team_name: None,
            external_id: None,
        },
    ];
    let res = bundle.import_subjects(&entries).unwrap();
    assert_eq!((res.imported_new, res.updated, res.auto_matched_photos), (2, 0, 1));

    let res2 = bundle.import_subjects(&entries).unwrap();
    assert_eq!((res2.imported_new, res2.updated), (0, 2));

    let unassigned = bundle.list_folders(None, true, None, None).unwrap();
    assert_eq!(unassigned.folders.len(), 2);
    assert_eq!(unassigned.total_unassigned_photos, 2);

    let avery = bundle.list_subjects(Some("143"), None, 10).unwrap().pop().unwrap();

    // Assign both folders to Avery -> second safely merges into 143 Avery W
    let a1 = bundle
        .apply_folder_reassignments(
            &[FolderReassignmentInput {
                folder_path: "Gym A/Session 1A/Group 1A/Beam/Gymnast 05".into(),
                target_subject_id: Some(avery.id.clone()),
            }],
            true,
        )
        .unwrap();
    assert_eq!(a1.photos_updated, 1);
    assert!(a1.renamed_on_disk);
    assert!(root.join("Gym A/Session 1A/Group 1A/Beam/143 Avery W/IMG_2.jpg").is_file());

    let a2 = bundle
        .apply_folder_reassignments(
            &[FolderReassignmentInput {
                folder_path: "Gym A/Session 1A/Group 1A/Beam/Gymnast 06".into(),
                target_subject_id: Some(avery.id.clone()),
            }],
            true,
        )
        .unwrap();
    assert_eq!(a2.photos_updated, 1);
    assert_eq!(a2.changes[0].action, "merged");
    assert!(root.join("Gym A/Session 1A/Group 1A/Beam/143 Avery W/IMG_3.jpg").is_file());

    let after = bundle.list_folders(None, true, None, None).unwrap();
    assert_eq!(after.folders.len(), 0);
}

#[test]
fn batch_reassign_handles_cyclic_rotation_swap_and_safe_merge_with_collision() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("rotation-swap");
    let bundle = test_bundle(root.clone(), "Rotation Swap Test", "ROT");

    let sub_a = bundle.add_subject(Some("105"), "Julia", Some("B"), Some("Session 10A"), None).unwrap();
    let sub_b = bundle.add_subject(Some("107"), "Reece", Some("C"), Some("Session 10A"), None).unwrap();
    let sub_c = bundle.add_subject(Some("117"), "Faith", Some("M"), Some("Session 10A"), None).unwrap();

    for (rel, bytes) in [
        ("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001.jpg", b"actually-reece".as_slice()),
        ("Gym A/Session 10A/Group 1A/Beam/107 Reece C/IMG_001.jpg", b"actually-faith".as_slice()),
        ("Gym A/Session 10A/Group 1A/Beam/117 Faith M/IMG_001.jpg", b"actually-julia".as_slice()),
        ("Gym A/Session 10A/Group 1A/Floor/105 Julia B/FLR_001.jpg", b"julia-floor".as_slice()),
    ] {
        write_test_file(&root.join(rel), bytes);
    }
    let scan = bundle.scan_directory().unwrap();
    assert_eq!(scan.photos_new, 4);
    for job in &scan.pending_jobs {
        bundle
            .mark_photo_ready(
                &job.photo_id,
                &format!("_photofinish/thumbnails/{}.jpg", job.photo_id),
                &format!("_photofinish/previews/{}.jpg", job.photo_id),
                800,
                600,
            )
            .unwrap();
    }

    let listed = bundle
        .list_folders(Some("Gym A/Session 10A/Group 1A/Beam"), false, None, None)
        .unwrap();
    assert_eq!(listed.folders.len(), 3);
    assert_eq!(listed.detected_session_group.as_deref(), Some("Session 10A"));
    assert_eq!(listed.detected_group_prefix.as_deref(), Some("Gym A/Session 10A/Group 1A"));
    assert_eq!(listed.same_group_subject_ids.len(), 3);

    let julia_floor_job = scan
        .pending_jobs
        .iter()
        .find(|j| j.relative_path.contains("FLR_001.jpg"))
        .unwrap();
    assert_eq!(listed.subject_sample_photos.get(&sub_a.id), Some(&julia_floor_job.photo_id));

    // Cyclic 3-way swap: 105 -> 107, 107 -> 117, 117 -> 105
    let res = bundle
        .apply_folder_reassignments(
            &[
                FolderReassignmentInput {
                    folder_path: "Gym A/Session 10A/Group 1A/Beam/105 Julia B".into(),
                    target_subject_id: Some(sub_b.id.clone()),
                },
                FolderReassignmentInput {
                    folder_path: "Gym A/Session 10A/Group 1A/Beam/107 Reece C".into(),
                    target_subject_id: Some(sub_c.id.clone()),
                },
                FolderReassignmentInput {
                    folder_path: "Gym A/Session 10A/Group 1A/Beam/117 Faith M".into(),
                    target_subject_id: Some(sub_a.id.clone()),
                },
            ],
            true,
        )
        .unwrap();
    assert_eq!((res.folders_updated, res.photos_updated), (3, 3));
    assert!(res.renamed_on_disk);
    assert!(res.changes.iter().all(|c| c.action == "swapped"));

    assert_eq!(
        fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/107 Reece C/IMG_001.jpg")).unwrap(),
        b"actually-reece"
    );
    assert_eq!(
        fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/117 Faith M/IMG_001.jpg")).unwrap(),
        b"actually-faith"
    );
    assert_eq!(
        fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001.jpg")).unwrap(),
        b"actually-julia"
    );

    write_test_file(&root.join("Gym A/Session 10A/Group 1A/Beam/Extra/IMG_001.jpg"), b"julia-extra");
    bundle.scan_directory().unwrap();

    let merge_res = bundle
        .apply_folder_reassignments(
            &[FolderReassignmentInput {
                folder_path: "Gym A/Session 10A/Group 1A/Beam/Extra".into(),
                target_subject_id: Some(sub_a.id),
            }],
            true,
        )
        .unwrap();
    assert_eq!(merge_res.changes[0].action, "merged");
    assert_eq!(
        fs::read(root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B/IMG_001_2.jpg")).unwrap(),
        b"julia-extra"
    );
}

#[test]
fn remove_folder_supports_trash_permanent_delete_and_db_only() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("remove-folder-test");
    let bundle = test_bundle(root.clone(), "Remove Folder Test", "REM");

    let f_trash = root.join("Gym B/Session 2B/Group 1B/Beam/Delete/IMG_1.jpg");
    let f_perm = root.join("Gym B/Session 2B/Group 1B/Beam/Junk/IMG_2.jpg");
    let f_db = root.join("Gym B/Session 2B/Group 1B/Beam/KeepDisk/IMG_3.jpg");
    for p in [&f_trash, &f_perm, &f_db] {
        write_test_file(p, b"jpgdata");
    }
    assert_eq!(bundle.scan_directory().unwrap().photos_new, 3);

    let r1 = bundle
        .remove_folder("Gym B/Session 2B/Group 1B/Beam/Delete", RemoveFolderMode::Trash)
        .unwrap();
    assert_eq!(r1.photos_removed, 1);
    assert!(!f_trash.exists());
    assert!(root.join(r1.trashed_to.unwrap()).join("IMG_1.jpg").is_file());

    let r2 = bundle
        .remove_folder("Gym B/Session 2B/Group 1B/Beam/Junk", RemoveFolderMode::DeleteDisk)
        .unwrap();
    assert_eq!(r2.photos_removed, 1);
    assert!(!f_perm.parent().unwrap().exists());

    let r3 = bundle
        .remove_folder("Gym B/Session 2B/Group 1B/Beam/KeepDisk", RemoveFolderMode::DbOnly)
        .unwrap();
    assert_eq!(r3.photos_removed, 1);
    assert!(f_db.is_file());

    let rescan = bundle.scan_directory().unwrap();
    assert_eq!((rescan.photos_new, rescan.photos_found), (1, 1));
}

#[test]
fn order_desk_payment_usb_fulfillment_pricing_and_csv_export() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("orders-meet");
    let usb_dir = dir.path().join("mock-usb-drive");
    let bundle = test_bundle(root.clone(), "State Finals", "STF");

    let sub = bundle.add_subject(Some("105"), "Julia", Some("B"), Some("Session 1A"), None).unwrap();
    write_test_file(&root.join("Gym A/Session 1A/Group 1A/Beam/105 Julia B/IMG_1001.jpg"), b"beam-orig");
    write_test_file(&root.join("Gym A/Session 1A/Group 1A/Floor/105 Julia B/IMG_2002.jpg"), b"floor-orig");

    let scan = bundle.scan_directory().unwrap();
    for job in &scan.pending_jobs {
        bundle
            .mark_photo_ready(
                &job.photo_id,
                &format!("_photofinish/thumbnails/{}.jpg", job.photo_id),
                &format!("_photofinish/previews/{}.jpg", job.photo_id),
                1200,
                800,
            )
            .unwrap();
    }

    let products = bundle.list_products(true).unwrap();
    let full_prod = products.iter().find(|p| p.product_type == "full_digital").unwrap();
    let usb_prod = products.iter().find(|p| p.product_type == "usb_addon").unwrap();

    bundle
        .upsert_product(&ProductUpsertInput {
            id: Some(full_prod.id.clone()),
            product_type: full_prod.product_type.clone(),
            name: full_prod.name.clone(),
            size: None,
            price_cents: 9000,
            is_available: true,
            display_order: full_prod.display_order,
        })
        .unwrap();
    let updated = bundle
        .update_event_order_settings(&EventOrderSettingsInput {
            order_code: "FIN".into(),
            station_code: "B".into(),
            tax_rate_basis_points: 700,
        })
        .unwrap();
    assert_eq!((updated.order_code.as_str(), updated.tax_rate_basis_points), ("FIN", 700));

    let order = bundle
        .place_order(CreateOrderRequest {
            customer_name: "Joe Franklin".into(),
            customer_email: Some("joe@example.com".into()),
            customer_phone: Some("555-1234".into()),
            shipping_address: None,
            notes: Some("Pick up at desk".into()),
            items: vec![
                CreateOrderItemInput {
                    product_id: full_prod.id.clone(),
                    subject_id: Some(sub.id.clone()),
                    photo_id: None,
                    quantity: 1,
                },
                CreateOrderItemInput {
                    product_id: usb_prod.id.clone(),
                    subject_id: Some(sub.id),
                    photo_id: None,
                    quantity: 1,
                },
            ],
        })
        .unwrap();
    assert_eq!((order.order_number.as_str(), order.total_cents), ("FIN-B0001", 10700));

    let before = bundle.list_orders().unwrap();
    assert_eq!((before.stats.pending_payment_count, before.stats.pending_usb_count), (1, 1));

    let paid = bundle
        .update_order_payment(&UpdatePaymentRequest {
            order_id: order.id.clone(),
            payment_status: "paid".into(),
            payment_method: Some("square_qr".into()),
            payment_reference: Some("SQ-9876".into()),
            notes: None,
        })
        .unwrap();
    assert_eq!(paid.payment_status, "paid");
    assert!(paid.paid_at.is_some());

    let (plan, copy_items) = bundle.resolve_order_usb_copy_plan(&order.id).unwrap();
    assert_eq!((plan.total_photos, plan.missing_files, plan.subjects.len()), (2, 0, 1));

    let (copied_count, _, _) =
        crate::hardware::copy_order_files_to_usb(&usb_dir, &copy_items, |_| {}).unwrap();
    assert_eq!(copied_count, 2);
    assert_eq!(fs::read(usb_dir.join("105 Julia B/Beam/IMG_1001.jpg")).unwrap(), b"beam-orig");
    assert_eq!(fs::read(usb_dir.join("105 Julia B/Floor/IMG_2002.jpg")).unwrap(), b"floor-orig");
    assert_eq!(bundle.mark_usb_items_fulfilled(&order.id).unwrap(), 1);

    let after = bundle.list_orders().unwrap();
    assert_eq!((after.stats.paid_count, after.stats.pending_usb_count, after.stats.gross_paid_cents), (1, 0, 10700));

    let csv = orders::format_orders_csv(&[("State Finals".into(), after.orders)]);
    assert!(csv.contains("FIN-B0001") && csv.contains("Joe Franklin") && csv.contains("107.00"));
}

#[test]
fn multi_station_gather_and_consolidated_csv_merge() {
    let nas_dir = tempdir().unwrap();
    let station_a_root = nas_dir.path().join("Weekend-03/Station-A/2026 IL State");
    let station_b_root = nas_dir.path().join("Weekend-03/Station-B/2026 IL State");
    let station_c_root = nas_dir.path().join("Weekend-03/Station-C/2026 WI State");

    let bundle_a = EventBundle::create(CreateEventParams {
        root_path: station_a_root.clone(),
        name: "2026 IL State (Gym A)".into(),
        order_code: Some("ILS".into()),
        station_code: Some("A".into()),
        tax_rate_basis_points: Some(800),
        ..Default::default()
    })
    .unwrap();
    let bundle_b = EventBundle::create(CreateEventParams {
        root_path: station_b_root.clone(),
        name: "2026 IL State (Gym B)".into(),
        order_code: Some("ILS".into()),
        station_code: Some("B".into()),
        tax_rate_basis_points: Some(800),
        ..Default::default()
    })
    .unwrap();
    EventBundle::create(CreateEventParams {
        root_path: station_c_root.clone(),
        name: "2026 WI State".into(),
        order_code: Some("WIS".into()),
        station_code: Some("C".into()),
        tax_rate_basis_points: Some(550),
        ..Default::default()
    })
    .unwrap();

    for (bundle, bib, name, parent, ref_code) in [
        (&bundle_a, "101", "Ava", "Parent A", "SQ-A1"),
        (&bundle_b, "202", "Chloe", "Parent B", "SQ-B1"),
    ] {
        let prod = bundle.list_products(false).unwrap()[0].clone();
        let sub = bundle.add_subject(Some(bib), name, None, None, None).unwrap();
        let ord = bundle
            .place_order(CreateOrderRequest {
                customer_name: parent.into(),
                customer_email: Some("p@example.com".into()),
                customer_phone: None,
                shipping_address: None,
                notes: None,
                items: vec![CreateOrderItemInput {
                    product_id: prod.id,
                    subject_id: Some(sub.id),
                    photo_id: None,
                    quantity: 1,
                }],
            })
            .unwrap();
        bundle
            .update_order_payment(&UpdatePaymentRequest {
                order_id: ord.id,
                payment_status: "paid".into(),
                payment_method: Some("square_qr".into()),
                payment_reference: Some(ref_code.into()),
                notes: None,
            })
            .unwrap();
    }

    assert_eq!(EventBundle::discover_in_directory(nas_dir.path()).unwrap().len(), 3);

    let ov_a = EventBundle::inspect_readonly(&station_a_root).unwrap();
    let ov_b = EventBundle::inspect_readonly(&station_b_root).unwrap();
    let ov_c = EventBundle::inspect_readonly(&station_c_root).unwrap();

    assert_eq!((ov_a.station_code.as_str(), ov_b.station_code.as_str(), ov_c.station_code.as_str()), ("A", "B", "C"));
    assert_eq!(ov_a.order_numbers, vec!["ILS-A0001"]);
    assert_eq!(ov_b.order_numbers, vec!["ILS-B0001"]);
    assert!(ov_a.gross_paid_cents > 0 && ov_a.order_numbers[0] != ov_b.order_numbers[0]);

    let csv = orders::format_orders_csv(&[
        (ov_a.name, bundle_a.list_orders().unwrap().orders),
        (ov_b.name, bundle_b.list_orders().unwrap().orders),
    ]);
    assert!(csv.contains("ILS-A0001") && csv.contains("ILS-B0001") && csv.contains("Parent A") && csv.contains("Parent B"));
}

#[test]
fn scan_and_reconcile_prunes_deleted_folders_and_cached_derivatives() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("orphan-meet");
    let bundle = test_bundle(root.clone(), "Orphan Cleanup Meet", "OCM");

    let sub_keep = bundle.add_subject(Some("101"), "Maya", Some("K"), Some("Session 10A"), None).unwrap();
    let sub_del = bundle.add_subject(Some("180"), "Zoe", Some("D"), Some("Session 18A"), None).unwrap();

    for rel in [
        "Gym A/Session 10A/Group 1A/Beam/101 Maya K/IMG_001.jpg",
        "Gym A/Session 18A/Group 1A/Beam/180 Zoe D/IMG_002.jpg",
        "Gym A/Session 18A/Group 1A/Floor/180 Zoe D/IMG_003.jpg",
    ] {
        write_test_file(&root.join(rel), b"fake-jpeg-bytes");
    }

    let scan1 = bundle.scan_directory().unwrap();
    assert_eq!((scan1.photos_new, scan1.photos_removed), (3, 0));

    let mut del_thumbs = Vec::new();
    for job in &scan1.pending_jobs {
        let t_rel = format!("_photofinish/thumbnails/{}.jpg", job.photo_id);
        let p_rel = format!("_photofinish/previews/{}.jpg", job.photo_id);
        fs::write(root.join(&t_rel), b"thumb").unwrap();
        fs::write(root.join(&p_rel), b"prev").unwrap();
        bundle.mark_photo_ready(&job.photo_id, &t_rel, &p_rel, 1280, 853).unwrap();
        if job.relative_path.contains("Session 18A") {
            del_thumbs.push((t_rel, p_rel));
        }
    }

    let del_photo_id = bundle.list_subject_photos(&sub_del.id).unwrap()[0].id.clone();
    let single_prod = bundle
        .list_products(false)
        .unwrap()
        .into_iter()
        .find(|p| p.product_type == "single_digital")
        .unwrap();
    bundle
        .place_order(CreateOrderRequest {
            customer_name: "Order Parent".into(),
            customer_email: None,
            customer_phone: None,
            shipping_address: None,
            notes: None,
            items: vec![CreateOrderItemInput {
                product_id: single_prod.id,
                subject_id: Some(sub_del.id.clone()),
                photo_id: Some(del_photo_id),
                quantity: 1,
            }],
        })
        .unwrap();

    fs::remove_dir_all(root.join("Gym A/Session 18A")).unwrap();

    let scan2 = bundle.scan_directory().unwrap();
    assert_eq!((scan2.photos_found, scan2.photos_new, scan2.photos_skipped, scan2.photos_removed), (1, 0, 1, 2));
    assert_eq!(bundle.list_subject_photos(&sub_keep.id).unwrap().len(), 1);
    assert_eq!(bundle.list_subject_photos(&sub_del.id).unwrap().len(), 0);
    for (t_rel, p_rel) in &del_thumbs {
        assert!(!root.join(t_rel).exists() && !root.join(p_rel).exists());
    }
    assert_eq!(bundle.list_orders().unwrap().orders.len(), 1);
}

#[test]
fn targeted_reconcile_subpaths_handles_add_delete_and_rename_without_touching_siblings() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("targeted-meet");
    let bundle = test_bundle(root.clone(), "Targeted Reconcile Meet", "TRM");

    let sub105 = bundle.add_subject(Some("105"), "Julia", Some("B"), Some("Session 10A"), None).unwrap();
    write_test_file(&root.join("Gym A/Session 10A/Group 1A/Beam/101 Keep/IMG_001.jpg"), b"keep-jpg");
    let old_dir = root.join("Gym A/Session 10A/Group 1A/Beam/TempFolder");
    write_test_file(&old_dir.join("IMG_002.jpg"), b"rename-jpg");

    assert_eq!(bundle.scan_directory().unwrap().photos_new, 2);

    let new_dir = root.join("Gym A/Session 10A/Group 1A/Beam/105 Julia B");
    fs::rename(&old_dir, &new_dir).unwrap();
    fs::write(new_dir.join("IMG_IN_PROGRESS.jpg"), b"").unwrap();

    let res = bundle
        .reconcile_subpaths(&[
            "Gym A/Session 10A/Group 1A/Beam/TempFolder".into(),
            "Gym A/Session 10A/Group 1A/Beam/105 Julia B".into(),
        ])
        .unwrap();
    assert_eq!((res.photos_removed, res.photos_new, res.pending_jobs.len()), (1, 1, 1));
    assert_eq!(bundle.photo_counts().unwrap().total, 2);

    bundle
        .mark_photo_ready(
            &res.pending_jobs[0].photo_id,
            "_photofinish/thumbnails/t.jpg",
            "_photofinish/previews/p.jpg",
            1280,
            853,
        )
        .unwrap();
    assert_eq!(bundle.list_subject_photos(&sub105.id).unwrap().len(), 1);
}

#[test]
fn ingest_batch_queue_scoping_and_section_matched_narrowing() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("queue-meet");
    let bundle = test_bundle(root.clone(), "Queue Narrowing Meet", "QNM");

    let sub101 = bundle.add_subject(Some("101"), "Ava", Some("M"), Some("Session 10A"), None).unwrap();
    let sub102 = bundle.add_subject(Some("102"), "Bella", Some("S"), Some("Session 10A"), None).unwrap();

    let f1 = "Gym A/Session 10A/Group 1A/Beam/101 Ava M/IMG_001.jpg";
    let f2 = "Gym A/Session 10A/Group 2A/Beam/Gymnast 01/IMG_002.jpg";
    write_test_file(&root.join(f1), b"jpg1");
    write_test_file(&root.join(f2), b"jpg2");

    let (batch1, _) = bundle
        .record_ingest_batch(
            IngestBatchMeta {
                reader_id: Some("USB-BEAM".into()),
                reader_name: Some("Gym A · Beam".into()),
                volume_name: Some("CARD1".into()),
                photographer: Some("KDS".into()),
                source_path: None,
                destination_rel_path: "Gym A/Session 10A/Group 1A/Beam".into(),
                tags: BTreeMap::new(),
            },
            &[IngestFileInput {
                relative_path: f1.into(),
                original_filename: "IMG_001.jpg".into(),
                file_size_bytes: 4,
            }],
        )
        .unwrap();
    let (batch2, _) = bundle
        .record_ingest_batch(
            IngestBatchMeta {
                reader_id: Some("USB-BEAM".into()),
                reader_name: Some("Gym A · Beam".into()),
                volume_name: Some("CARD2".into()),
                photographer: Some("KDS".into()),
                source_path: None,
                destination_rel_path: "Gym A/Session 10A/Group 2A/Beam".into(),
                tags: BTreeMap::new(),
            },
            &[IngestFileInput {
                relative_path: f2.into(),
                original_filename: "IMG_002.jpg".into(),
                file_size_bytes: 4,
            }],
        )
        .unwrap();

    let batches = bundle.list_ingest_batches(20).unwrap();
    assert_eq!((batches[0].id.as_str(), batches[0].unassigned_folders), (batch2.as_str(), 1));
    assert_eq!((batches[1].id.as_str(), batches[1].unassigned_folders), (batch1.as_str(), 0));

    let view = bundle.list_folders(None, false, None, Some(&batch2)).unwrap();
    assert_eq!(view.folders.len(), 1);
    assert_eq!(view.section_label.as_deref(), Some("Beam"));
    assert_eq!(view.section_matched_subjects.get(&sub101.id).map(String::as_str), Some("Group 1A"));
    assert!(!view.section_matched_subjects.contains_key(&sub102.id));

    bundle
        .apply_folder_reassignments(
            &[FolderReassignmentInput {
                folder_path: "Gym A/Session 10A/Group 2A/Beam/Gymnast 01".into(),
                target_subject_id: Some(sub102.id.clone()),
            }],
            true,
        )
        .unwrap();
    assert_eq!(bundle.list_ingest_batches(20).unwrap()[0].unassigned_folders, 0);
    assert_eq!(bundle.list_folders(None, true, None, Some(&batch2)).unwrap().folders.len(), 0);
    let card_all = bundle.list_folders(None, false, None, Some(&batch2)).unwrap();
    assert_eq!(card_all.folders[0].assigned_subject_id.as_deref(), Some(sub102.id.as_str()));
}
