defmodule PhotoFinish.Ingestion.ScannerTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.Scanner

  describe "scan_directory/1" do
    test "finds JPEG files recursively" do
      # Create a temp directory structure
      tmp_dir = System.tmp_dir!() |> Path.join("scanner_test_#{:rand.uniform(10000)}")
      File.mkdir_p!(Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S"]))

      # Create test files
      jpeg_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "IMG_001.jpg"])
      File.write!(jpeg_path, "fake jpeg content")

      # Also create a non-JPEG to ensure it's filtered
      txt_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "notes.txt"])
      File.write!(txt_path, "some notes")

      try do
        {:ok, files} = Scanner.scan_directory(tmp_dir)

        assert length(files) == 1
        assert hd(files).path == jpeg_path
        assert hd(files).filename == "IMG_001.jpg"
      after
        File.rm_rf!(tmp_dir)
      end
    end

    test "returns error for non-existent directory" do
      result = Scanner.scan_directory("/non/existent/path")

      assert result == {:error, :directory_not_found}
    end
  end

  describe "file_signature/1" do
    test "creates signature from file stat" do
      tmp_file = System.tmp_dir!() |> Path.join("test_file_#{:rand.uniform(10000)}.jpg")
      File.write!(tmp_file, "test content")

      try do
        {:ok, sig} = Scanner.file_signature(tmp_file)

        assert sig.filename == Path.basename(tmp_file)
        assert sig.size == 12  # "test content" is 12 bytes
        assert is_integer(sig.mtime)
      after
        File.rm!(tmp_file)
      end
    end
  end
end
