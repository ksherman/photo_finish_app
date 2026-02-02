defmodule PhotoFinish.Events.EventCompetitorTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}

  describe "create" do
    setup do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event",
          storage_root: "/path/to/photos"
        })

      {:ok, competitor} =
        Ash.create(Competitor, %{
          first_name: "Jane",
          last_name: "Doe"
        })

      %{event: event, competitor: competitor}
    end

    test "generates ID with evc_ prefix", %{event: event, competitor: competitor} do
      {:ok, event_competitor} =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id,
          competitor_number: "101"
        })

      assert String.starts_with?(event_competitor.id, "evc_")
      suffix = String.replace_prefix(event_competitor.id, "evc_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates event_competitor with required fields", %{event: event, competitor: competitor} do
      {:ok, event_competitor} =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id,
          competitor_number: "202"
        })

      assert event_competitor.competitor_number == "202"
      assert event_competitor.event_id == event.id
      assert event_competitor.competitor_id == competitor.id
    end

    test "creates event_competitor with session and all optional fields", %{event: event, competitor: competitor} do
      {:ok, event_competitor} =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id,
          competitor_number: "303",
          session: "3A",
          display_name: "Jane D.",
          team_name: "Elite Gymnastics",
          level: "Level 7",
          age_group: "Junior",
          is_active: true,
          metadata: %{"custom_field" => "value"}
        })

      assert event_competitor.session == "3A"
      assert event_competitor.display_name == "Jane D."
      assert event_competitor.team_name == "Elite Gymnastics"
      assert event_competitor.level == "Level 7"
      assert event_competitor.age_group == "Junior"
      assert event_competitor.is_active == true
      assert event_competitor.metadata == %{"custom_field" => "value"}
    end

    test "defaults is_active to true", %{event: event, competitor: competitor} do
      {:ok, event_competitor} =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id,
          competitor_number: "404"
        })

      assert event_competitor.is_active == true
    end

    test "requires competitor_number", %{event: event, competitor: competitor} do
      result =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id
        })

      assert {:error, _} = result
    end

    test "enforces unique competitor_number per event", %{event: event, competitor: competitor} do
      {:ok, _} =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor.id,
          competitor_number: "505"
        })

      # Create another competitor to avoid same competitor_id
      {:ok, competitor2} =
        Ash.create(Competitor, %{
          first_name: "John",
          last_name: "Smith"
        })

      # Should fail with same competitor_number in same event
      result =
        Ash.create(EventCompetitor, %{
          event_id: event.id,
          competitor_id: competitor2.id,
          competitor_number: "505"
        })

      assert {:error, _} = result
    end

    test "allows same competitor_number in different events", %{competitor: competitor} do
      {:ok, event1} =
        Ash.create(Event, %{
          name: "Event 1",
          slug: "event-1",
          storage_root: "/photos/event1"
        })

      {:ok, event2} =
        Ash.create(Event, %{
          name: "Event 2",
          slug: "event-2",
          storage_root: "/photos/event2"
        })

      {:ok, ec1} =
        Ash.create(EventCompetitor, %{
          event_id: event1.id,
          competitor_id: competitor.id,
          competitor_number: "606"
        })

      {:ok, ec2} =
        Ash.create(EventCompetitor, %{
          event_id: event2.id,
          competitor_id: competitor.id,
          competitor_number: "606"
        })

      assert ec1.competitor_number == "606"
      assert ec2.competitor_number == "606"
      assert ec1.event_id != ec2.event_id
    end
  end
end
