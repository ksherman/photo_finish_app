defmodule PhotoFinish.Ingestion.PathParserTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.PathParser

  describe "parse/2" do
    test "parses a valid full path" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)

      assert result == %{
               gym: "A",
               session: "1A",
               group_name: "Group 2B",
               apparatus: "Beam",
               competitor_folder: "1059 Iza Z",
               filename: "IMG_001.jpg"
             }
    end

    test "handles storage root with trailing slash" do
      storage_root = "/storage/"
      full_path = "/storage/Gym B/Session 2C/Group 3D/Vault/2001 Jane D/photo.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)

      assert result.gym == "B"
      assert result.session == "2C"
      assert result.group_name == "Group 3D"
      assert result.apparatus == "Vault"
      assert result.competitor_folder == "2001 Jane D"
      assert result.filename == "photo.jpg"
    end

    test "handles multi-character gym identifier" do
      storage_root = "/storage"
      full_path = "/storage/Gym BC/Session 1A/Group 2B/Floor/1022 Kevin S/IMG_002.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)
      assert result.gym == "BC"
    end

    test "handles multi-character session identifier" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Session 12B/Group 2B/Bars/1022 Kevin S/IMG_003.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)
      assert result.session == "12B"
    end

    test "preserves full group name" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Session 1A/Group Level 4/Beam/1022 Kevin S/IMG_004.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)
      assert result.group_name == "Group Level 4"
    end

    test "returns error for path with too few segments" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Session 1A/photo.jpg"

      assert {:error, :invalid_path} = PathParser.parse(full_path, storage_root)
    end

    test "returns error for path with too many segments" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Session 1A/Group 2B/Beam/Extra/1022 Kevin S/IMG_001.jpg"

      assert {:error, :invalid_path} = PathParser.parse(full_path, storage_root)
    end

    test "returns error for invalid gym format" do
      storage_root = "/storage"
      full_path = "/storage/Location A/Session 1A/Group 2B/Beam/1022 Kevin S/IMG_001.jpg"

      assert {:error, :invalid_path} = PathParser.parse(full_path, storage_root)
    end

    test "returns error for invalid session format" do
      storage_root = "/storage"
      full_path = "/storage/Gym A/Period 1A/Group 2B/Beam/1022 Kevin S/IMG_001.jpg"

      assert {:error, :invalid_path} = PathParser.parse(full_path, storage_root)
    end

    test "handles deep storage root path" do
      storage_root = "/events/2024/gymnastics/meet1"

      full_path =
        "/events/2024/gymnastics/meet1/Gym A/Session 1A/Group 2B/Beam/1022 Kevin S/IMG_001.jpg"

      assert {:ok, result} = PathParser.parse(full_path, storage_root)

      assert result.gym == "A"
      assert result.session == "1A"
      assert result.apparatus == "Beam"
    end

    test "handles various apparatus names" do
      storage_root = "/storage"

      for apparatus <- ["Beam", "Vault", "Floor", "Bars", "Uneven Bars", "Parallel Bars"] do
        full_path = "/storage/Gym A/Session 1A/Group 2B/#{apparatus}/1022 Kevin S/IMG_001.jpg"

        assert {:ok, result} = PathParser.parse(full_path, storage_root)
        assert result.apparatus == apparatus
      end
    end

    test "handles competitor folder with various formats" do
      storage_root = "/storage"

      test_cases = [
        "1059 Iza Z",
        "1022 Kevin Sherman",
        "999 A B",
        "12345 Long Name Here"
      ]

      for folder <- test_cases do
        full_path = "/storage/Gym A/Session 1A/Group 2B/Beam/#{folder}/IMG_001.jpg"

        assert {:ok, result} = PathParser.parse(full_path, storage_root)
        assert result.competitor_folder == folder
      end
    end

    test "handles various filename formats" do
      storage_root = "/storage"

      filenames = [
        "IMG_001.jpg",
        "IMG_0001.JPG",
        "photo.jpeg",
        "DSC_1234.JPEG",
        "image-with-dashes.jpg"
      ]

      for filename <- filenames do
        full_path = "/storage/Gym A/Session 1A/Group 2B/Beam/1022 Kevin S/#{filename}"

        assert {:ok, result} = PathParser.parse(full_path, storage_root)
        assert result.filename == filename
      end
    end
  end

  describe "parse_gym/1" do
    test "extracts letter from valid gym folder" do
      assert {:ok, "A"} = PathParser.parse_gym("Gym A")
      assert {:ok, "B"} = PathParser.parse_gym("Gym B")
      assert {:ok, "BC"} = PathParser.parse_gym("Gym BC")
    end

    test "returns error for invalid gym format" do
      assert {:error, :invalid_gym} = PathParser.parse_gym("Location A")
      assert {:error, :invalid_gym} = PathParser.parse_gym("GymA")
      assert {:error, :invalid_gym} = PathParser.parse_gym("gym a")
      assert {:error, :invalid_gym} = PathParser.parse_gym("Gym ")
      assert {:error, :invalid_gym} = PathParser.parse_gym("")
    end
  end

  describe "parse_session/1" do
    test "extracts identifier from valid session folder" do
      assert {:ok, "1A"} = PathParser.parse_session("Session 1A")
      assert {:ok, "2B"} = PathParser.parse_session("Session 2B")
      assert {:ok, "12C"} = PathParser.parse_session("Session 12C")
      assert {:ok, "1"} = PathParser.parse_session("Session 1")
    end

    test "returns error for invalid session format" do
      assert {:error, :invalid_session} = PathParser.parse_session("Period 1A")
      assert {:error, :invalid_session} = PathParser.parse_session("Session1A")
      assert {:error, :invalid_session} = PathParser.parse_session("session 1A")
      assert {:error, :invalid_session} = PathParser.parse_session("Session ")
      assert {:error, :invalid_session} = PathParser.parse_session("")
    end
  end
end
