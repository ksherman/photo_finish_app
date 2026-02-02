defmodule PhotoFinish.Ingestion.ScannerTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.Scanner

  describe "scan_directory/1" do
    test "finds JPEG files recursively" do
      # Create a temp directory structure (avoid spaces in paths for consistency)
      tmp_dir = System.tmp_dir!() |> Path.join("scanner_test_#{:rand.uniform(10000)}")
      nested_dir = Path.join([tmp_dir, "gym", "session", "competitor"])
      File.mkdir_p!(nested_dir)

      # Create test files
      jpeg_path = Path.join(nested_dir, "IMG_001.jpg")
      File.write!(jpeg_path, "fake jpeg content")

      # Verify file was created
      assert File.exists?(jpeg_path), "JPEG file should exist at #{jpeg_path}"

      # Also create a non-JPEG to ensure it's filtered
      txt_path = Path.join(nested_dir, "notes.txt")
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
        # "test content" is 12 bytes
        assert sig.size == 12
        assert is_integer(sig.mtime)
      after
        File.rm!(tmp_file)
      end
    end
  end
end
