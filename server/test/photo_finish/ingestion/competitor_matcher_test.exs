defmodule PhotoFinish.Ingestion.CompetitorMatcherTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.CompetitorMatcher

  describe "extract_competitor_number/1" do
    test "extracts number from folder name" do
      assert CompetitorMatcher.extract_competitor_number("1022 Kevin S") == {:ok, "1022"}
      assert CompetitorMatcher.extract_competitor_number("123 Jane Doe") == {:ok, "123"}
      assert CompetitorMatcher.extract_competitor_number("9999 A") == {:ok, "9999"}
    end

    test "returns no_match for non-matching patterns" do
      assert CompetitorMatcher.extract_competitor_number("Gymnast 01") == :no_match
      assert CompetitorMatcher.extract_competitor_number("Floor") == :no_match
      assert CompetitorMatcher.extract_competitor_number("Group 1A") == :no_match
    end

    test "handles edge cases" do
      assert CompetitorMatcher.extract_competitor_number("1022") == :no_match
      assert CompetitorMatcher.extract_competitor_number("") == :no_match
      assert CompetitorMatcher.extract_competitor_number("  1022 Kevin") == {:ok, "1022"}
    end
  end
end
