defmodule PhotoFinish.Events.FolderAssociation do
  @moduledoc """
  Handles associating photo folders (source_folder) to event_competitors.

  This module provides functions for:
  - Listing unassigned photo folders at a given location
  - Listing event_competitors in a session
  - Bulk-assigning all photos in a folder to an event_competitor
  """

  require Logger
  import Ecto.Query
  require Ash.Query

  alias PhotoFinish.Repo
  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Events.EventCompetitor

  @doc """
  List folders (source_folder values) at a given location that have
  photos not yet assigned to an event_competitor.

  Returns a list of maps with :source_folder and :photo_count keys.

  ## Parameters

    - event_id: The event ID to filter by
    - location: A map with :gym, :session, :group_name, and :apparatus keys

  ## Examples

      iex> list_unassigned_folders(event_id, %{
      ...>   gym: "A",
      ...>   session: "3A",
      ...>   group_name: "Group 3A",
      ...>   apparatus: "Beam"
      ...> })
      [%{source_folder: "1022 Jane D", photo_count: 12}, ...]

  """
  def list_unassigned_folders(event_id, %{
        gym: gym,
        session: session,
        group_name: group_name,
        apparatus: apparatus
      }) do
    from(p in Photo,
      where: p.event_id == ^event_id,
      where: p.gym == ^gym,
      where: p.session == ^session,
      where: p.group_name == ^group_name,
      where: p.apparatus == ^apparatus,
      where: is_nil(p.event_competitor_id),
      where: not is_nil(p.source_folder),
      group_by: p.source_folder,
      select: %{source_folder: p.source_folder, photo_count: count(p.id)},
      order_by: p.source_folder
    )
    |> Repo.all()
  end

  @doc """
  List event_competitors for a given event and session.

  Returns a list of EventCompetitor structs sorted by competitor_number.

  ## Parameters

    - event_id: The event ID to filter by
    - session: The session identifier (e.g., "3A", "11B")

  ## Examples

      iex> list_session_event_competitors(event_id, "3A")
      [%EventCompetitor{competitor_number: "1022", ...}, ...]

  """
  def list_session_event_competitors(event_id, session) do
    EventCompetitor
    |> Ash.Query.filter(event_id == ^event_id and session == ^session)
    |> Ash.read!()
    |> Enum.sort_by(&parse_competitor_number/1)
  end

  # Sort competitor numbers numerically (handles mixed number/text like "123" or "123A")
  defp parse_competitor_number(ec) do
    case Integer.parse(ec.competitor_number || "") do
      {num, _rest} -> num
      :error -> 0
    end
  end

  @doc """
  Assign all photos in a source_folder to an event_competitor.

  Renames the physical directory on disk to match the competitor's
  number and display name, and updates photo records accordingly.

  Only updates photos that don't already have an event_competitor_id assigned.

  Returns {:ok, count} with the number of photos updated.

  ## Parameters

    - event_id: The event ID to filter by
    - source_folder: The source_folder value to match
    - competitor: The EventCompetitor struct (needs :id, :competitor_number, :display_name)
    - location: Map with :storage_root, :gym, :session, :group_name, :apparatus

  """
  def assign_folder(event_id, source_folder, competitor, location) do
    %{
      storage_root: storage_root,
      gym: gym,
      session: session,
      group_name: group_name,
      apparatus: apparatus
    } = location

    new_folder_name = competitor.display_name
    rename? = source_folder != new_folder_name

    # Rename physical directory on disk
    if rename? do
      rename_directory(storage_root, gym, session, group_name, apparatus, source_folder, new_folder_name)
    end

    # Build path segments for ingestion_path replacement
    old_segment = "/#{source_folder}/"
    new_segment = "/#{new_folder_name}/"
    now = DateTime.utc_now()

    query =
      from(p in Photo,
        where: p.event_id == ^event_id,
        where: p.source_folder == ^source_folder,
        where: p.gym == ^gym,
        where: p.session == ^session,
        where: p.group_name == ^group_name,
        where: p.apparatus == ^apparatus,
        where: is_nil(p.event_competitor_id)
      )

    {count, _} =
      if rename? do
        query
        |> Repo.update_all(
          set: [
            event_competitor_id: competitor.id,
            source_folder: new_folder_name,
            updated_at: now
          ]
        )
      else
        query
        |> Repo.update_all(
          set: [
            event_competitor_id: competitor.id,
            updated_at: now
          ]
        )
      end

    # Update ingestion_path with string replacement (needs fragment, so separate query)
    if rename? && count > 0 do
      from(p in Photo,
        where: p.event_id == ^event_id,
        where: p.event_competitor_id == ^competitor.id,
        where: p.gym == ^gym,
        where: p.session == ^session,
        where: p.group_name == ^group_name,
        where: p.apparatus == ^apparatus,
        update: [
          set: [
            ingestion_path: fragment("REPLACE(?, ?, ?)", p.ingestion_path, ^old_segment, ^new_segment)
          ]
        ]
      )
      |> Repo.update_all([])
    end

    {:ok, count}
  end

  defp rename_directory(storage_root, gym, session, group_name, apparatus, old_name, new_name) do
    base = Path.join([storage_root, "Gym #{gym}", "Session #{session}", group_name, apparatus])
    old_dir = Path.join(base, old_name)
    new_dir = Path.join(base, new_name)

    if File.dir?(old_dir) do
      case File.rename(old_dir, new_dir) do
        :ok ->
          Logger.info("Renamed folder #{old_name} -> #{new_name}")

        {:error, reason} ->
          Logger.warning("Failed to rename folder #{old_name} -> #{new_name}: #{inspect(reason)}")
      end
    else
      Logger.warning("Directory not found for rename: #{old_dir}")
    end
  end
end
