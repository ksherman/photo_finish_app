defmodule PhotoFinish.Events.RosterParserTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Events.RosterParser

  describe "parse_txt/1" do
    test "parses simple roster format" do
      content = """
      143 Avery W
      169 Callie W
      1022 Kevin S
      """

      {:ok, competitors} = RosterParser.parse_txt(content)

      assert length(competitors) == 3

      assert Enum.at(competitors, 0) == %{
               competitor_number: "143",
               first_name: "Avery",
               last_name: "W"
             }
    end

    test "handles single-word names" do
      content = "143 Avery\n"

      {:ok, [competitor]} = RosterParser.parse_txt(content)

      assert competitor.first_name == "Avery"
      assert competitor.last_name == nil
    end

    test "skips blank lines" do
      content = "143 Avery W\n\n169 Callie W\n"

      {:ok, competitors} = RosterParser.parse_txt(content)
      assert length(competitors) == 2
    end

    test "returns error for invalid lines" do
      content = "not a valid line\n"

      assert {:error, _} = RosterParser.parse_txt(content)
    end
  end
end
