defmodule PhotoFinish.Events.EventTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Event

  describe "create" do
    test "generates ID with evt_ prefix" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event",
          storage_root: "/path/to/photos"
        })

      assert String.starts_with?(event.id, "evt_")
      suffix = String.replace_prefix(event.id, "evt_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates event with storage_root, num_gyms, sessions_per_gym" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Regional Meet 2024",
          slug: "regional-2024",
          storage_root: "/nas/photos/regional-2024",
          num_gyms: 3,
          sessions_per_gym: 4
        })

      assert event.name == "Regional Meet 2024"
      assert event.slug == "regional-2024"
      assert event.storage_root == "/nas/photos/regional-2024"
      assert event.num_gyms == 3
      assert event.sessions_per_gym == 4
    end

    test "uses default values for num_gyms and sessions_per_gym" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Small Event",
          slug: "small-event",
          storage_root: "/nas/photos/small"
        })

      assert event.num_gyms == 1
      assert event.sessions_per_gym == 1
    end

    test "requires storage_root" do
      result =
        Ash.create(Event, %{
          name: "Missing Storage",
          slug: "missing-storage"
        })

      assert {:error, _} = result
    end
  end

  describe "update" do
    test "can update storage_root, num_gyms, sessions_per_gym" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event",
          storage_root: "/original/path"
        })

      {:ok, updated} =
        Ash.update(event, %{
          storage_root: "/new/path",
          num_gyms: 2,
          sessions_per_gym: 3
        })

      assert updated.storage_root == "/new/path"
      assert updated.num_gyms == 2
      assert updated.sessions_per_gym == 3
    end
  end
end
