defmodule PhotoFinish.Viewer.Search do
  @moduledoc """
  Search functionality for the public viewer.
  Searches event_competitors by number, first name, or last name.
  """

  import Ecto.Query
  alias PhotoFinish.Repo
  alias PhotoFinish.Events.{Competitor, EventCompetitor}
  alias PhotoFinish.Photos.Photo

  @max_results 10

  @doc """
  Search event_competitors by number, first name, or last name.
  Returns event_competitors with photo counts (only ready photos counted).

  ## Examples

      iex> search_event_competitors(event_id, "1022")
      [%{id: "evc_...", competitor_number: "1022", display_name: "1022 Kevin S", ...}]

      iex> search_event_competitors(event_id, "kevin")
      [%{id: "evc_...", competitor_number: "1022", display_name: "1022 Kevin S", ...}]

  """
  def search_event_competitors(event_id, query) when is_binary(query) do
    query = String.trim(query)

    if String.length(query) < 1 do
      []
    else
      pattern = "%#{query}%"

      from(ec in EventCompetitor,
        join: c in Competitor,
        on: ec.competitor_id == c.id,
        where: ec.event_id == ^event_id,
        where: ec.is_active == true,
        where:
          ilike(ec.competitor_number, ^pattern) or
            ilike(c.first_name, ^pattern) or
            ilike(c.last_name, ^pattern),
        left_join: p in Photo,
        on: p.event_competitor_id == ec.id and p.status == :ready,
        group_by: [ec.id, c.first_name, c.last_name],
        select: %{
          id: ec.id,
          competitor_number: ec.competitor_number,
          display_name: ec.display_name,
          session: ec.session,
          first_name: c.first_name,
          last_name: c.last_name,
          photo_count: count(p.id)
        },
        order_by: ec.competitor_number,
        limit: @max_results
      )
      |> Repo.all()
    end
  end
end
