defmodule PhotoFinish.Events.FolderAssociationTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{FolderAssociation, Competitor, EventCompetitor, Event}
  alias PhotoFinish.Photos.Photo

  describe "list_unassigned_folders/2" do
    test "returns folders with unassigned photos" do
      {:ok, event} = create_test_event()

      {:ok, _} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, _} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, _} = create_photo(event.id, nil, "Gym 02", "A", "3A", "Group 3A", "Beam")

      folders =
        FolderAssociation.list_unassigned_folders(event.id, %{
          gym: "A",
          session: "3A",
          group_name: "Group 3A",
          apparatus: "Beam"
        })

      assert length(folders) == 2
      assert Enum.find(folders, &(&1.source_folder == "Gym 01")).photo_count == 2
      assert Enum.find(folders, &(&1.source_folder == "Gym 02")).photo_count == 1
    end

    test "excludes folders with assigned photos" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")

      # One folder with assigned photos (should be excluded)
      {:ok, _} = create_photo(event.id, ec.id, "Assigned Folder", "A", "3A", "Group 3A", "Beam")

      # One folder with unassigned photos (should be included)
      {:ok, _} = create_photo(event.id, nil, "Unassigned Folder", "A", "3A", "Group 3A", "Beam")

      folders =
        FolderAssociation.list_unassigned_folders(event.id, %{
          gym: "A",
          session: "3A",
          group_name: "Group 3A",
          apparatus: "Beam"
        })

      assert length(folders) == 1
      assert hd(folders).source_folder == "Unassigned Folder"
    end

    test "excludes photos with nil source_folder" do
      {:ok, event} = create_test_event()

      {:ok, _} = create_photo(event.id, nil, nil, "A", "3A", "Group 3A", "Beam")
      {:ok, _} = create_photo(event.id, nil, "Valid Folder", "A", "3A", "Group 3A", "Beam")

      folders =
        FolderAssociation.list_unassigned_folders(event.id, %{
          gym: "A",
          session: "3A",
          group_name: "Group 3A",
          apparatus: "Beam"
        })

      assert length(folders) == 1
      assert hd(folders).source_folder == "Valid Folder"
    end

    test "filters by location correctly" do
      {:ok, event} = create_test_event()

      # Same folder name but different locations
      {:ok, _} = create_photo(event.id, nil, "Folder X", "A", "3A", "Group 3A", "Beam")
      {:ok, _} = create_photo(event.id, nil, "Folder X", "B", "3A", "Group 3A", "Beam")

      folders_gym_a =
        FolderAssociation.list_unassigned_folders(event.id, %{
          gym: "A",
          session: "3A",
          group_name: "Group 3A",
          apparatus: "Beam"
        })

      folders_gym_b =
        FolderAssociation.list_unassigned_folders(event.id, %{
          gym: "B",
          session: "3A",
          group_name: "Group 3A",
          apparatus: "Beam"
        })

      assert length(folders_gym_a) == 1
      assert length(folders_gym_b) == 1
    end
  end

  describe "list_session_event_competitors/2" do
    test "returns event_competitors for session" do
      {:ok, event} = create_test_event()
      {:ok, _ec1} = create_event_competitor(event.id, "3A", "1022")
      {:ok, _ec2} = create_event_competitor(event.id, "3A", "1023")
      {:ok, _ec3} = create_event_competitor(event.id, "4A", "1024")

      result = FolderAssociation.list_session_event_competitors(event.id, "3A")

      assert length(result) == 2
      assert Enum.all?(result, &(&1.session == "3A"))
    end

    test "returns empty list when no competitors in session" do
      {:ok, event} = create_test_event()
      {:ok, _ec1} = create_event_competitor(event.id, "4A", "1024")

      result = FolderAssociation.list_session_event_competitors(event.id, "3A")

      assert result == []
    end

    test "sorts by competitor_number" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor(event.id, "3A", "1025")
      {:ok, _} = create_event_competitor(event.id, "3A", "1022")
      {:ok, _} = create_event_competitor(event.id, "3A", "1023")

      result = FolderAssociation.list_session_event_competitors(event.id, "3A")
      numbers = Enum.map(result, & &1.competitor_number)

      assert numbers == ["1022", "1023", "1025"]
    end
  end

  describe "assign_folder/3" do
    test "sets event_competitor_id on photos in folder" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")
      {:ok, p1} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, p2} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, p3} = create_photo(event.id, nil, "Gym 02", "A", "3A", "Group 3A", "Beam")

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", ec.id)

      assert count == 2

      # Verify photos were updated
      updated1 = Ash.get!(Photo, p1.id)
      updated2 = Ash.get!(Photo, p2.id)
      updated3 = Ash.get!(Photo, p3.id)

      assert updated1.event_competitor_id == ec.id
      assert updated2.event_competitor_id == ec.id
      assert updated3.event_competitor_id == nil
    end

    test "only updates photos without existing assignment" do
      {:ok, event} = create_test_event()
      {:ok, ec1} = create_event_competitor(event.id, "3A", "1022")
      {:ok, ec2} = create_event_competitor(event.id, "3A", "1023")

      # Photo already assigned to ec1
      {:ok, p1} = create_photo(event.id, ec1.id, "Gym 01", "A", "3A", "Group 3A", "Beam")
      # Unassigned photo
      {:ok, p2} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", ec2.id)

      assert count == 1

      updated1 = Ash.get!(Photo, p1.id)
      updated2 = Ash.get!(Photo, p2.id)

      # p1 should still be assigned to ec1
      assert updated1.event_competitor_id == ec1.id
      # p2 should now be assigned to ec2
      assert updated2.event_competitor_id == ec2.id
    end

    test "returns zero when no matching photos" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Nonexistent Folder", ec.id)

      assert count == 0
    end
  end

  # Helper functions
  defp create_test_event do
    Ash.create(Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end

  defp create_event_competitor(event_id, session, number) do
    {:ok, competitor} = Ash.create(Competitor, %{first_name: "Test"})

    Ash.create(EventCompetitor, %{
      competitor_id: competitor.id,
      event_id: event_id,
      session: session,
      competitor_number: number,
      display_name: "#{number} Test"
    })
  end

  defp create_photo(
         event_id,
         event_competitor_id,
         source_folder,
         gym,
         session,
         group_name,
         apparatus
       ) do
    Ash.create(Photo, %{
      event_id: event_id,
      event_competitor_id: event_competitor_id,
      source_folder: source_folder,
      gym: gym,
      session: session,
      group_name: group_name,
      apparatus: apparatus,
      ingestion_path: "/tmp/#{System.unique_integer([:positive])}.jpg",
      filename: "photo_#{System.unique_integer([:positive])}.jpg"
    })
  end
end
