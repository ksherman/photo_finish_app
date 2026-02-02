defmodule PhotoFinish.Ingestion.CompetitorMatcher do
  @moduledoc """
  Matches folder names to competitors in the roster.
  """

  # Pattern: starts with digits, followed by space and name
  @competitor_pattern ~r/^\s*(\d+)\s+\S/

  @doc """
  Extracts competitor number from a folder name.

  Expects format: "{number} {name}" (e.g., "1022 Kevin S")

  Returns {:ok, number} or :no_match
  """
  @spec extract_competitor_number(String.t()) :: {:ok, String.t()} | :no_match
  def extract_competitor_number(folder_name) do
    case Regex.run(@competitor_pattern, folder_name) do
      [_, number] -> {:ok, number}
      _ -> :no_match
    end
  end

  @doc """
  Finds a competitor by number in the given list.

  Returns {:ok, competitor} or :no_match
  """
  @spec find_competitor([map()], String.t()) :: {:ok, map()} | :no_match
  def find_competitor(competitors, competitor_number) do
    case Enum.find(competitors, &(&1.competitor_number == competitor_number)) do
      nil -> :no_match
      competitor -> {:ok, competitor}
    end
  end

  @doc """
  Finds an event competitor by number in the given list.

  Returns {:ok, event_competitor} or :no_match
  """
  @spec find_event_competitor([map()], String.t()) :: {:ok, map()} | :no_match
  def find_event_competitor(event_competitors, competitor_number) do
    case Enum.find(event_competitors, &(&1.competitor_number == competitor_number)) do
      nil -> :no_match
      event_competitor -> {:ok, event_competitor}
    end
  end
end
