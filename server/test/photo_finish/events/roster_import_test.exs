defmodule PhotoFinish.Events.RosterImportTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{RosterImport, Competitor, EventCompetitor}

  describe "import_roster/3" do
    test "creates competitor and event_competitor records" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery W
      169 Callie W
      """

      {:ok, result} = RosterImport.import_roster(event.id, "3A", content)

      assert result.imported_count == 2

      # Verify both tables were populated
      competitors = Ash.read!(Competitor)
      event_competitors = Ash.read!(EventCompetitor)

      assert length(competitors) == 2
      assert length(event_competitors) == 2

      avery_ec = Enum.find(event_competitors, &(&1.competitor_number == "143"))
      assert avery_ec.session == "3A"
      assert avery_ec.display_name == "143 Avery W"
    end

    test "returns error count for invalid lines" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery W
      invalid line without number
      169 Callie W
      """

      {:error, _} = RosterImport.import_roster(event.id, "3A", content)
    end

    test "handles single name (no last name)" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery
      """

      {:ok, result} = RosterImport.import_roster(event.id, "3A", content)

      assert result.imported_count == 1

      event_competitors = Ash.read!(EventCompetitor)
      assert length(event_competitors) == 1

      ec = hd(event_competitors)
      assert ec.display_name == "143 Avery"
    end

    test "associates all event_competitors with correct event" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery W
      169 Callie W
      """

      {:ok, _result} = RosterImport.import_roster(event.id, "3A", content)

      event_competitors = Ash.read!(EventCompetitor)

      Enum.each(event_competitors, fn ec ->
        assert ec.event_id == event.id
      end)
    end

    test "creates competitor records with first and last names" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery Williams
      """

      {:ok, _result} = RosterImport.import_roster(event.id, "3A", content)

      competitors = Ash.read!(Competitor)
      assert length(competitors) == 1

      competitor = hd(competitors)
      assert competitor.first_name == "Avery"
      assert competitor.last_name == "Williams"
    end
  end

  defp create_test_event do
    Ash.create(PhotoFinish.Events.Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end
end
