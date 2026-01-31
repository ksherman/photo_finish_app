defmodule PhotoFinish.Events.FolderGeneratorTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Events.FolderGenerator

  describe "gym_letter/1" do
    test "converts 1 to A" do
      assert FolderGenerator.gym_letter(1) == "A"
    end

    test "converts 2 to B" do
      assert FolderGenerator.gym_letter(2) == "B"
    end

    test "converts 26 to Z" do
      assert FolderGenerator.gym_letter(26) == "Z"
    end

    test "converts sequential numbers to letters" do
      assert FolderGenerator.gym_letter(3) == "C"
      assert FolderGenerator.gym_letter(10) == "J"
      assert FolderGenerator.gym_letter(13) == "M"
    end

    test "gym_letter/1 raises FunctionClauseError for gym_num > 26" do
      assert_raise FunctionClauseError, fn -> FolderGenerator.gym_letter(27) end
    end

    test "gym_letter/1 raises FunctionClauseError for gym_num < 1" do
      assert_raise FunctionClauseError, fn -> FolderGenerator.gym_letter(0) end
    end
  end

  describe "create_event_folders/3" do
    setup do
      # Create a unique temp directory for each test
      temp_dir =
        Path.join(System.tmp_dir!(), "photo_finish_test_#{:erlang.unique_integer([:positive])}")

      on_exit(fn ->
        # Clean up the temp directory after the test
        File.rm_rf!(temp_dir)
      end)

      {:ok, temp_dir: temp_dir}
    end

    test "creates folder structure for 1 gym with 2 sessions", %{temp_dir: temp_dir} do
      {:ok, paths} = FolderGenerator.create_event_folders(temp_dir, 1, 2)

      # Check that paths are returned
      assert length(paths) == 2

      # Verify the session folders exist
      assert File.dir?(Path.join([temp_dir, "Gym A", "Session 1A"]))
      assert File.dir?(Path.join([temp_dir, "Gym A", "Session 2A"]))
    end

    test "creates folder structure for 3 gyms with 4 sessions each", %{temp_dir: temp_dir} do
      {:ok, paths} = FolderGenerator.create_event_folders(temp_dir, 3, 4)

      # Check that we created 3 gyms * 4 sessions = 12 paths
      assert length(paths) == 12

      # Verify all gym folders exist
      assert File.dir?(Path.join(temp_dir, "Gym A"))
      assert File.dir?(Path.join(temp_dir, "Gym B"))
      assert File.dir?(Path.join(temp_dir, "Gym C"))

      # Verify all session folders exist
      for gym_num <- 1..3, session_num <- 1..4 do
        letter = FolderGenerator.gym_letter(gym_num)
        gym_folder = "Gym #{letter}"
        session_folder = "Session #{session_num}#{letter}"
        path = Path.join([temp_dir, gym_folder, session_folder])
        assert File.dir?(path), "Expected #{path} to exist"
      end
    end

    test "creates storage_root if it doesn't exist", %{temp_dir: temp_dir} do
      nested_root = Path.join([temp_dir, "nested", "path", "event"])

      refute File.exists?(nested_root)

      {:ok, _paths} = FolderGenerator.create_event_folders(nested_root, 1, 1)

      assert File.dir?(nested_root)
      assert File.dir?(Path.join([nested_root, "Gym A", "Session 1A"]))
    end

    test "returns the list of created session paths", %{temp_dir: temp_dir} do
      {:ok, paths} = FolderGenerator.create_event_folders(temp_dir, 2, 2)

      expected_paths = [
        Path.join([temp_dir, "Gym A", "Session 1A"]),
        Path.join([temp_dir, "Gym A", "Session 2A"]),
        Path.join([temp_dir, "Gym B", "Session 1B"]),
        Path.join([temp_dir, "Gym B", "Session 2B"])
      ]

      assert Enum.sort(paths) == Enum.sort(expected_paths)
    end

    test "is idempotent - can be called multiple times without error", %{temp_dir: temp_dir} do
      {:ok, paths1} = FolderGenerator.create_event_folders(temp_dir, 2, 2)
      {:ok, paths2} = FolderGenerator.create_event_folders(temp_dir, 2, 2)

      assert Enum.sort(paths1) == Enum.sort(paths2)
    end

    test "handles single gym and single session", %{temp_dir: temp_dir} do
      {:ok, paths} = FolderGenerator.create_event_folders(temp_dir, 1, 1)

      assert paths == [Path.join([temp_dir, "Gym A", "Session 1A"])]
      assert File.dir?(Path.join([temp_dir, "Gym A", "Session 1A"]))
    end
  end
end
