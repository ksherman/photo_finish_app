defmodule PhotoFinish.Viewer.SearchTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Viewer.Search
  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}
  alias PhotoFinish.Photos.Photo

  describe "search_event_competitors/2" do
    test "finds by competitor number" do
      {:ok, event} = create_test_event()
      {:ok, _ec1} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "S", 5)
      {:ok, _ec2} = create_event_competitor_with_photos(event.id, "1023", "Sarah", "J", 3)

      results = Search.search_event_competitors(event.id, "1022")

      assert length(results) == 1
      assert hd(results).competitor_number == "1022"
      assert hd(results).photo_count == 5
    end

    test "finds by first name (case insensitive)" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "Sherman", 5)

      results = Search.search_event_competitors(event.id, "kevin")

      assert length(results) == 1
    end

    test "finds by last name" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "Sherman", 5)

      results = Search.search_event_competitors(event.id, "sherman")

      assert length(results) == 1
    end

    test "returns empty list when no matches" do
      {:ok, event} = create_test_event()

      results = Search.search_event_competitors(event.id, "nobody")

      assert results == []
    end

    test "limits results to 10" do
      {:ok, event} = create_test_event()

      for i <- 1..15 do
        create_event_competitor_with_photos(event.id, "10#{i}", "Test", "#{i}", 1)
      end

      results = Search.search_event_competitors(event.id, "Test")

      assert length(results) == 10
    end

    test "only counts ready photos" do
      {:ok, event} = create_test_event()
      {:ok, competitor} = Ash.create(Competitor, %{first_name: "Kevin", last_name: "S"})

      {:ok, ec} =
        Ash.create(EventCompetitor, %{
          competitor_id: competitor.id,
          event_id: event.id,
          competitor_number: "1022",
          session: "1A",
          display_name: "1022 Kevin S"
        })

      # Create 3 ready photos and 2 non-ready
      for _ <- 1..3 do
        create_photo(event.id, ec.id, :ready)
      end

      for _ <- 1..2 do
        create_photo(event.id, ec.id, :processing)
      end

      results = Search.search_event_competitors(event.id, "Kevin")

      assert length(results) == 1
      assert hd(results).photo_count == 3
    end

    test "excludes inactive event competitors" do
      {:ok, event} = create_test_event()
      {:ok, competitor} = Ash.create(Competitor, %{first_name: "Kevin", last_name: "S"})

      {:ok, _ec} =
        Ash.create(EventCompetitor, %{
          competitor_id: competitor.id,
          event_id: event.id,
          competitor_number: "1022",
          session: "1A",
          display_name: "1022 Kevin S",
          is_active: false
        })

      results = Search.search_event_competitors(event.id, "Kevin")

      assert results == []
    end

    test "returns empty list for empty query" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "S", 5)

      results = Search.search_event_competitors(event.id, "")

      assert results == []
    end

    test "returns empty list for whitespace-only query" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "S", 5)

      results = Search.search_event_competitors(event.id, "   ")

      assert results == []
    end

    test "returns result with zero photo count when no ready photos exist" do
      {:ok, event} = create_test_event()
      {:ok, competitor} = Ash.create(Competitor, %{first_name: "Kevin", last_name: "S"})

      {:ok, _ec} =
        Ash.create(EventCompetitor, %{
          competitor_id: competitor.id,
          event_id: event.id,
          competitor_number: "1022",
          session: "1A",
          display_name: "1022 Kevin S"
        })

      results = Search.search_event_competitors(event.id, "Kevin")

      assert length(results) == 1
      assert hd(results).photo_count == 0
    end

    test "returns all expected fields" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_event_competitor_with_photos(event.id, "1022", "Kevin", "Sherman", 3)

      [result] = Search.search_event_competitors(event.id, "Kevin")

      assert String.starts_with?(result.id, "evc_")
      assert result.competitor_number == "1022"
      assert result.display_name == "1022 Kevin Sherman"
      assert result.session == "1A"
      assert result.first_name == "Kevin"
      assert result.last_name == "Sherman"
      assert result.photo_count == 3
    end
  end

  # Helpers

  defp create_test_event do
    Ash.create(Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end

  defp create_event_competitor_with_photos(event_id, number, first, last, photo_count) do
    {:ok, competitor} = Ash.create(Competitor, %{first_name: first, last_name: last})

    {:ok, ec} =
      Ash.create(EventCompetitor, %{
        competitor_id: competitor.id,
        event_id: event_id,
        competitor_number: number,
        session: "1A",
        display_name: "#{number} #{first} #{last}"
      })

    for _ <- 1..photo_count do
      create_photo(event_id, ec.id, :ready)
    end

    {:ok, ec}
  end

  defp create_photo(event_id, event_competitor_id, status) do
    Ash.create!(Photo, %{
      event_id: event_id,
      event_competitor_id: event_competitor_id,
      ingestion_path: "/tmp/#{System.unique_integer([:positive])}.jpg",
      filename: "photo.jpg",
      status: status
    })
  end
end
