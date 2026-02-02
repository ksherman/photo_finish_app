defmodule PhotoFinish.Events.CompetitorTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Competitor

  describe "create" do
    test "generates ID with cmp_ prefix" do
      {:ok, competitor} =
        Ash.create(Competitor, %{
          first_name: "Jane"
        })

      assert String.starts_with?(competitor.id, "cmp_")
      suffix = String.replace_prefix(competitor.id, "cmp_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates competitor with required fields" do
      {:ok, competitor} =
        Ash.create(Competitor, %{
          first_name: "John"
        })

      assert competitor.first_name == "John"
    end

    test "creates competitor with all optional fields" do
      {:ok, competitor} =
        Ash.create(Competitor, %{
          first_name: "Sarah",
          last_name: "Smith",
          email: "sarah@example.com",
          phone: "555-1234",
          external_id: "ext-123",
          metadata: %{"custom_field" => "value"}
        })

      assert competitor.first_name == "Sarah"
      assert competitor.last_name == "Smith"
      assert competitor.email == "sarah@example.com"
      assert competitor.phone == "555-1234"
      assert competitor.external_id == "ext-123"
      assert competitor.metadata == %{"custom_field" => "value"}
    end

    test "requires first_name" do
      result =
        Ash.create(Competitor, %{
          last_name: "Smith"
        })

      assert {:error, _} = result
    end
  end
end
