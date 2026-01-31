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

  describe "build_output_path/3" do
    test "builds thumbnail path" do
      result =
        PhotoProcessor.build_output_path(
          "/NAS/thumbnails",
          "valentines-2025",
          "photo-uuid-123"
        )

      assert result == "/NAS/thumbnails/valentines-2025/photo-uuid-123.jpg"
    end
  end
end
