defmodule PhotoFinish.Photos.PhotoTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Events.Event

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/path/to/photos"
      })

    {:ok, event: event}
  end

  describe "create" do
    test "generates ID with pho_ prefix", %{event: event} do
      {:ok, photo} =
        Ash.create(Photo, %{
          event_id: event.id,
          ingestion_path: "/path/to/image.jpg",
          filename: "image.jpg"
        })

      assert String.starts_with?(photo.id, "pho_")
      suffix = String.replace_prefix(photo.id, "pho_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates photo with location fields", %{event: event} do
      {:ok, photo} =
        Ash.create(Photo, %{
          event_id: event.id,
          ingestion_path: "/path/to/image.jpg",
          filename: "image.jpg",
          gym: "A",
          session: "1A",
          group_name: "Group 1A",
          apparatus: "Beam"
        })

      assert photo.gym == "A"
      assert photo.session == "1A"
      assert photo.group_name == "Group 1A"
      assert photo.apparatus == "Beam"
    end

    test "location fields are optional (can be nil)", %{event: event} do
      {:ok, photo} =
        Ash.create(Photo, %{
          event_id: event.id,
          ingestion_path: "/path/to/image.jpg",
          filename: "image.jpg"
        })

      assert photo.gym == nil
      assert photo.session == nil
      assert photo.group_name == nil
      assert photo.apparatus == nil
    end

    test "creates photo with partial location fields", %{event: event} do
      {:ok, photo} =
        Ash.create(Photo, %{
          event_id: event.id,
          ingestion_path: "/path/to/image.jpg",
          filename: "image.jpg",
          gym: "B",
          apparatus: "Floor"
        })

      assert photo.gym == "B"
      assert photo.session == nil
      assert photo.group_name == nil
      assert photo.apparatus == "Floor"
    end
  end

  describe "update" do
    test "can update location fields", %{event: event} do
      {:ok, photo} =
        Ash.create(Photo, %{
          event_id: event.id,
          ingestion_path: "/path/to/image.jpg",
          filename: "image.jpg"
        })

      {:ok, updated} =
        Ash.update(photo, %{
          gym: "C",
          session: "2B",
          group_name: "Group 2B",
          apparatus: "Vault"
        })

      assert updated.gym == "C"
      assert updated.session == "2B"
      assert updated.group_name == "Group 2B"
      assert updated.apparatus == "Vault"
    end
  end
end
