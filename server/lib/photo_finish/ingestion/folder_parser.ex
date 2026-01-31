defmodule PhotoFinish.Ingestion.FolderParser do
  @moduledoc """
  Parses folder paths relative to an event's storage directory
  into hierarchy level tuples.
  """

  @doc """
  Parses a full path relative to the storage root.

  Returns a list of {level_number, folder_name} tuples.

  ## Examples

      iex> parse_path("/NAS/events/meet/Gym A/Session 1", "/NAS/events/meet")
      [{1, "Gym A"}, {2, "Session 1"}]
  """
  @spec parse_path(String.t(), String.t()) ::
          [{pos_integer(), String.t()}] | {:error, :path_not_under_root}
  def parse_path(full_path, storage_root) do
    normalized_path = String.trim_trailing(full_path, "/")
    normalized_root = String.trim_trailing(storage_root, "/")

    case String.replace_prefix(normalized_path, normalized_root, "") do
      ^normalized_path ->
        # Path didn't change, meaning it's not under root
        {:error, :path_not_under_root}

      "" ->
        # Path equals root
        []

      relative ->
        relative
        |> String.trim_leading("/")
        |> String.split("/")
        |> Enum.with_index(1)
        |> Enum.map(fn {name, index} -> {index, name} end)
    end
  end

  @doc """
  Converts a folder name to a URL-safe slug.
  """
  @spec slugify(String.t()) :: String.t()
  def slugify(name) do
    name
    |> String.downcase()
    |> String.replace(~r/[^a-z0-9\s-]/, "")
    |> String.replace(~r/\s+/, "-")
    |> String.trim("-")
  end
end
