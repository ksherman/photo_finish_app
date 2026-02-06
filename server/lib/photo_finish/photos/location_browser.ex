defmodule PhotoFinish.Photos.LocationBrowser do
  @moduledoc """
  Provides hierarchical browsing of photos by location (gym/session/group/apparatus).

  At the leaf level (apparatus), photos are grouped by source_folder for accordion display.
  """

  import Ecto.Query

  alias PhotoFinish.Repo
  alias PhotoFinish.Photos.Photo

  @levels [:gym, :session, :group_name, :apparatus]

  @doc """
  Returns the hierarchy levels in order.
  """
  def levels, do: @levels

  @doc """
  Returns the current level based on path depth.
  """
  def current_level(path) when length(path) < 4 do
    Enum.at(@levels, length(path))
  end

  def current_level(_path), do: nil

  @doc """
  Returns the display name for a level.
  """
  def level_label(:gym), do: "Gym"
  def level_label(:session), do: "Session"
  def level_label(:group_name), do: "Group"
  def level_label(:apparatus), do: "Apparatus"
  def level_label(_), do: nil

  @doc """
  Formats a value for display based on its level.
  E.g., "A" at gym level becomes "Gym A", "1A" at session level becomes "Session 1A"
  """
  def format_value(:gym, value), do: "Gym #{value}"
  def format_value(:session, value), do: "Session #{value}"
  def format_value(_level, value), do: value

  @doc """
  Formats a value at a given path index for display.
  """
  def format_value_at_index(value, index) when index < 4 do
    level = Enum.at(@levels, index)
    format_value(level, value)
  end

  def format_value_at_index(value, _index), do: value

  @doc """
  Gets children at the current level with photo counts.

  Returns a list of maps: %{name: "Gym A", count: 45}
  """
  def get_children(event_id, path) do
    level = current_level(path)

    if level do
      event_id
      |> base_query(path)
      |> group_by_level(level)
      |> Repo.all()
      |> Enum.map(fn {name, count} -> %{name: name || "(unassigned)", count: count} end)
      |> Enum.sort_by(& &1.name)
    else
      []
    end
  end

  @doc """
  Gets photos grouped by source_folder (only when at leaf level).

  Returns a list of maps: %{folder: "1059 Iza Z", count: 12, photos: [...]}
  """
  def get_photo_folders(event_id, path) when length(path) == 4 do
    photos =
      event_id
      |> base_query(path)
      |> order_by([p], asc: p.source_folder, asc: p.filename)
      |> Repo.all()

    photos
    |> Enum.group_by(& &1.source_folder)
    |> Enum.map(fn {folder, folder_photos} ->
      %{
        folder: folder || "(unassigned)",
        count: length(folder_photos),
        photos: folder_photos
      }
    end)
    |> Enum.sort_by(& &1.folder)
  end

  def get_photo_folders(_event_id, _path), do: []

  @doc """
  Checks if we're at the leaf level (should show photo folders).
  """
  def at_leaf_level?(path), do: length(path) == 4

  defp base_query(event_id, path) do
    query = from(p in Photo, where: p.event_id == ^event_id)

    path
    |> Enum.with_index()
    |> Enum.reduce(query, fn {value, index}, q ->
      level = Enum.at(@levels, index)
      where(q, [p], field(p, ^level) == ^value)
    end)
  end

  defp group_by_level(query, level) do
    query
    |> group_by([p], field(p, ^level))
    |> select([p], {field(p, ^level), count(p.id)})
  end
end
