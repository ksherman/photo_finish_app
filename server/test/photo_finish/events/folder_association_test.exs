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

  describe "assign_folder/4" do
    test "sets event_competitor_id on photos in folder" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")
      {:ok, p1} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, p2} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, p3} = create_photo(event.id, nil, "Gym 02", "A", "3A", "Group 3A", "Beam")

      location = test_location(event)

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", ec, location)

      assert count == 2

      updated1 = Ash.get!(Photo, p1.id)
      updated2 = Ash.get!(Photo, p2.id)
      updated3 = Ash.get!(Photo, p3.id)

      assert updated1.event_competitor_id == ec.id
      assert updated2.event_competitor_id == ec.id
      assert updated3.event_competitor_id == nil
    end

    test "updates source_folder and ingestion_path on rename" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")

      {:ok, p1} =
        create_photo_with_path(
          event.id,
          nil,
          "Gymnast 00",
          "A",
          "3A",
          "Group 3A",
          "Beam",
          "/tmp/test/Gym A/Session 3A/Group 3A/Beam/Gymnast 00/IMG_001.jpg"
        )

      location = test_location(event)

      {:ok, 1} = FolderAssociation.assign_folder(event.id, "Gymnast 00", ec, location)

      updated = Ash.get!(Photo, p1.id)
      assert updated.source_folder == "1022 Test"
      assert updated.ingestion_path == "/tmp/test/Gym A/Session 3A/Group 3A/Beam/1022 Test/IMG_001.jpg"
    end

    test "only updates photos without existing assignment" do
      {:ok, event} = create_test_event()
      {:ok, ec1} = create_event_competitor(event.id, "3A", "1022")
      {:ok, ec2} = create_event_competitor(event.id, "3A", "1023")

      {:ok, p1} = create_photo(event.id, ec1.id, "Gym 01", "A", "3A", "Group 3A", "Beam")
      {:ok, p2} = create_photo(event.id, nil, "Gym 01", "A", "3A", "Group 3A", "Beam")

      location = test_location(event)

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", ec2, location)

      assert count == 1

      updated1 = Ash.get!(Photo, p1.id)
      updated2 = Ash.get!(Photo, p2.id)

      assert updated1.event_competitor_id == ec1.id
      assert updated2.event_competitor_id == ec2.id
    end

    test "returns zero when no matching photos" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")

      location = test_location(event)

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Nonexistent Folder", ec, location)

      assert count == 0
    end

    test "scopes assignment to specific location" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")

      # Same source_folder name in two different apparatus directories
      {:ok, p_beam} = create_photo(event.id, nil, "Gymnast 00", "A", "3A", "Group 3A", "Beam")
      {:ok, p_floor} = create_photo(event.id, nil, "Gymnast 00", "A", "3A", "Group 3A", "Floor")

      location = test_location(event)

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gymnast 00", ec, location)

      assert count == 1

      assert Ash.get!(Photo, p_beam.id).event_competitor_id == ec.id
      assert Ash.get!(Photo, p_floor.id).event_competitor_id == nil
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

  defp create_photo_with_path(
         event_id,
         event_competitor_id,
         source_folder,
         gym,
         session,
         group_name,
         apparatus,
         ingestion_path
       ) do
    Ash.create(Photo, %{
      event_id: event_id,
      event_competitor_id: event_competitor_id,
      source_folder: source_folder,
      gym: gym,
      session: session,
      group_name: group_name,
      apparatus: apparatus,
      ingestion_path: ingestion_path,
      filename: Path.basename(ingestion_path)
    })
  end

  defp test_location(event) do
    %{
      storage_root: event.storage_root,
      gym: "A",
      session: "3A",
      group_name: "Group 3A",
      apparatus: "Beam"
    }
  end
end
