defmodule PhotoFinish.Ingestion.FolderParserTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.FolderParser

  describe "parse_path/2" do
    test "parses folder path relative to storage root" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/events/valentines-2025/Gym A/Session 3/Group 1A/Beam/1022 Kevin S"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == [
               {1, "Gym A"},
               {2, "Session 3"},
               {3, "Group 1A"},
               {4, "Beam"},
               {5, "1022 Kevin S"}
             ]
    end

    test "handles trailing slashes" do
      storage_root = "/NAS/events/valentines-2025/"
      full_path = "/NAS/events/valentines-2025/Gym A/Session 1/"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == [
               {1, "Gym A"},
               {2, "Session 1"}
             ]
    end

    test "returns empty list for root path" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/events/valentines-2025"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == []
    end

    test "returns error if path not under root" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/other/path"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == {:error, :path_not_under_root}
    end
  end

  describe "slugify/1" do
    test "converts name to slug" do
      assert FolderParser.slugify("Gym A") == "gym-a"
      assert FolderParser.slugify("Session 3") == "session-3"
      assert FolderParser.slugify("1022 Kevin S") == "1022-kevin-s"
    end
  end
end
