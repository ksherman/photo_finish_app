defmodule PhotoFinish.Events.RosterImport do
  @moduledoc """
  Imports competitor rosters from file content.
  Creates both Competitor (person) and EventCompetitor (event participation) records.
  """

  alias PhotoFinish.Events.{Competitor, EventCompetitor, RosterParser}

  @doc """
  Imports a roster from text content for a given event and session.

  Creates a Competitor record for each person and an EventCompetitor record
  linking them to the event with the specified session.

  ## Parameters

    - event_id: The ID of the event to import into
    - session: The session identifier (e.g., "3A", "11B")
    - content: Raw text content with one competitor per line (format: "NUMBER NAME")

  ## Returns

    - `{:ok, %{imported_count: integer, error_count: integer}}` on success
    - `{:error, reason}` if parsing fails
  """
  def import_roster(event_id, session, content) when is_binary(content) do
    with {:ok, parsed} <- RosterParser.parse_txt(content) do
      results =
        Enum.map(parsed, fn data ->
          create_competitor_pair(event_id, session, data)
        end)

      imported = Enum.count(results, &match?({:ok, _}, &1))
      errors = Enum.count(results, &match?({:error, _}, &1))

      {:ok, %{imported_count: imported, error_count: errors}}
    end
  end

  defp create_competitor_pair(event_id, session, %{
         competitor_number: number,
         first_name: first,
         last_name: last
       }) do
    # Create the person record
    with {:ok, competitor} <-
           Ash.create(Competitor, %{
             first_name: first,
             last_name: last
           }) do
      # Create the event participation record
      display_name = build_display_name(number, first, last)

      Ash.create(EventCompetitor, %{
        competitor_id: competitor.id,
        event_id: event_id,
        session: session,
        competitor_number: number,
        display_name: display_name
      })
    end
  end

  defp build_display_name(number, first, nil), do: "#{number} #{first}"
  defp build_display_name(number, first, last), do: "#{number} #{first} #{last}"
end
