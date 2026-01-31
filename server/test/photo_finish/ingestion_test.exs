defmodule PhotoFinish.IngestionTest do
  use PhotoFinish.DataCase, async: false

  alias PhotoFinish.Ingestion

  describe "scan_event/1" do
    setup do
      # Create a temp directory structure
      tmp_dir = System.tmp_dir!() |> Path.join("ingestion_test_#{:rand.uniform(100_000)}")
      File.mkdir_p!(Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S"]))

      # Create test JPEG
      jpeg_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "IMG_001.jpg"])
      File.write!(jpeg_path, "fake jpeg content for testing")

      # Create an event with storage_directory pointing to tmp_dir
      {:ok, event} =
        Ash.create(PhotoFinish.Events.Event, %{
          name: "Test Event",
          slug: "test-event",
          storage_root: tmp_dir
        })

      on_exit(fn -> File.rm_rf!(tmp_dir) end)

      %{event: event, tmp_dir: tmp_dir}
    end

    test "creates photo records", %{event: event} do
      {:ok, _result} = Ingestion.scan_event(event.id)

      photos =
        Ash.read!(PhotoFinish.Photos.Photo)
        |> Enum.filter(&(&1.event_id == event.id))

      assert length(photos) == 1
      photo = hd(photos)
      assert photo.filename == "IMG_001.jpg"
      assert photo.status == :discovered
    end

    test "is idempotent - skips existing photos", %{event: event} do
      {:ok, result1} = Ingestion.scan_event(event.id)
      assert result1.photos_new == 1

      {:ok, result2} = Ingestion.scan_event(event.id)
      assert result2.photos_new == 0
      assert result2.photos_skipped == 1
    end

    test "returns error for missing storage directory" do
      {:ok, event} =
        Ash.create(PhotoFinish.Events.Event, %{
          name: "No Storage",
          slug: "no-storage",
          storage_root: "/non/existent/path"
        })

      result = Ingestion.scan_event(event.id)

      assert {:error, :directory_not_found} = result
    end
  end
end
