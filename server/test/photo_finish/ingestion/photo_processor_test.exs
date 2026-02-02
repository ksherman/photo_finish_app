defmodule PhotoFinish.Ingestion.PhotoProcessorTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.PhotoProcessor

  describe "new/1" do
    test "creates a valid Oban job" do
      changeset = PhotoProcessor.new(%{photo_id: "abc-123"})

      assert changeset.changes.args == %{photo_id: "abc-123"}
      assert changeset.changes.queue == "media"
    end
  end

  describe "thumbnail_path/2" do
    test "builds thumbnail path within storage root" do
      result = PhotoProcessor.thumbnail_path("/NAS/photos/event-123", "photo-uuid-123")

      assert result == "/NAS/photos/event-123/_thumbnails/photo-uuid-123.jpg"
    end
  end

  describe "preview_path/2" do
    test "builds preview path within storage root" do
      result = PhotoProcessor.preview_path("/NAS/photos/event-123", "photo-uuid-123")

      assert result == "/NAS/photos/event-123/_previews/photo-uuid-123.jpg"
    end
  end
end
