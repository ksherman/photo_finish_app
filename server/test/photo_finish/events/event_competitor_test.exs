defmodule PhotoFinish.Events.EventCompetitorTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}

  describe "create" do
    test "creates event_competitor linking person to event" do
      {:ok, event} = Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/tmp/test"
      })

      {:ok, competitor} = Ash.create(Competitor, %{
        first_name: "Kevin",
        last_name: "S"
      })

      {:ok, event_competitor} = Ash.create(EventCompetitor, %{
        competitor_id: competitor.id,
        event_id: event.id,
        competitor_number: "1022",
        session: "3A",
        display_name: "1022 Kevin S"
      })

      assert event_competitor.session == "3A"
      assert event_competitor.competitor_number == "1022"
      assert String.starts_with?(event_competitor.id, "evc_")
    end

    test "enforces unique competitor_number per event" do
      {:ok, event} = Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/tmp/test"
      })

      {:ok, c1} = Ash.create(Competitor, %{first_name: "Kevin"})
      {:ok, c2} = Ash.create(Competitor, %{first_name: "Sarah"})

      {:ok, _} = Ash.create(EventCompetitor, %{
        competitor_id: c1.id,
        event_id: event.id,
        competitor_number: "1022"
      })

      # Should fail - same competitor_number in same event
      assert {:error, _} = Ash.create(EventCompetitor, %{
        competitor_id: c2.id,
        event_id: event.id,
        competitor_number: "1022"
      })
    end
  end
end
