defmodule PhotoFinish.Events.CompetitorTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Competitor
  alias PhotoFinish.Events.Event

  describe "create" do
    test "generates ID with cmp_ prefix" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event",
          storage_root: "/path/to/photos"
        })

      {:ok, competitor} =
        Ash.create(Competitor, %{
          event_id: event.id,
          competitor_number: "101",
          first_name: "Jane"
        })

      assert String.starts_with?(competitor.id, "cmp_")
      suffix = String.replace_prefix(competitor.id, "cmp_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates competitor with required fields" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Regional Meet",
          slug: "regional-meet",
          storage_root: "/nas/photos"
        })

      {:ok, competitor} =
        Ash.create(Competitor, %{
          event_id: event.id,
          competitor_number: "202",
          first_name: "John"
        })

      assert competitor.competitor_number == "202"
      assert competitor.first_name == "John"
      assert competitor.event_id == event.id
    end

    test "creates competitor with all optional fields" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "State Championship",
          slug: "state-champ",
          storage_root: "/nas/photos/state"
        })

      {:ok, competitor} =
        Ash.create(Competitor, %{
          event_id: event.id,
          competitor_number: "303",
          first_name: "Sarah",
          last_name: "Smith",
          display_name: "Sarah S.",
          team_name: "Elite Gymnastics",
          level: "Level 7",
          age_group: "Junior",
          email: "sarah@example.com",
          phone: "555-1234",
          is_active: true,
          metadata: %{"custom_field" => "value"}
        })

      assert competitor.last_name == "Smith"
      assert competitor.display_name == "Sarah S."
      assert competitor.team_name == "Elite Gymnastics"
      assert competitor.level == "Level 7"
      assert competitor.age_group == "Junior"
      assert competitor.email == "sarah@example.com"
      assert competitor.phone == "555-1234"
      assert competitor.is_active == true
      assert competitor.metadata == %{"custom_field" => "value"}
    end

    test "defaults is_active to true" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event-2",
          storage_root: "/path/to/photos"
        })

      {:ok, competitor} =
        Ash.create(Competitor, %{
          event_id: event.id,
          competitor_number: "404",
          first_name: "Alex"
        })

      assert competitor.is_active == true
    end

    test "can create competitor without event_id" do
      {:ok, competitor} =
        Ash.create(Competitor, %{
          competitor_number: "505",
          first_name: "Test"
        })

      assert competitor.event_id == nil
      assert String.starts_with?(competitor.id, "cmp_")
    end

    test "requires competitor_number" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event-3",
          storage_root: "/path/to/photos"
        })

      result =
        Ash.create(Competitor, %{
          event_id: event.id,
          first_name: "Test"
        })

      assert {:error, _} = result
    end

    test "requires first_name" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Test Event",
          slug: "test-event-4",
          storage_root: "/path/to/photos"
        })

      result =
        Ash.create(Competitor, %{
          event_id: event.id,
          competitor_number: "606"
        })

      assert {:error, _} = result
    end
  end
end
