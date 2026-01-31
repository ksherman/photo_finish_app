defmodule PhotoFinish.Ingestion.PathParser do
  @moduledoc """
  Parses photo file paths to extract location information.

  Expected path format after storage_root:
    /Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg

  Where:
  - Index 0: Gym folder ("Gym A" -> gym: "A")
  - Index 1: Session folder ("Session 1A" -> session: "1A")
  - Index 2: Group folder ("Group 2B" -> group_name: "Group 2B")
  - Index 3: Apparatus folder ("Beam" -> apparatus: "Beam")
  - Index 4: Competitor folder ("1059 Iza Z" -> competitor_folder: "1059 Iza Z")
  - Index 5: Filename ("IMG_001.jpg" -> filename: "IMG_001.jpg")
  """

  @type parsed_path :: %{
          gym: String.t(),
          session: String.t(),
          group_name: String.t(),
          apparatus: String.t(),
          competitor_folder: String.t(),
          filename: String.t()
        }

  @doc """
  Parses a photo path relative to the event storage root.

  Returns a map with:
  - gym: "A" (letter only, extracted from "Gym A")
  - session: "1A" (number + letter, extracted from "Session 1A")
  - group_name: "Group 2B" (full name)
  - apparatus: "Beam"
  - competitor_folder: "1059 Iza Z"
  - filename: "IMG_001.jpg"

  Returns {:error, :invalid_path} if path doesn't match expected format.

  ## Examples

      iex> parse("/storage/Gym A/Session 1A/Group 2B/Beam/1059 Iza Z/IMG_001.jpg", "/storage")
      {:ok, %{
        gym: "A",
        session: "1A",
        group_name: "Group 2B",
        apparatus: "Beam",
        competitor_folder: "1059 Iza Z",
        filename: "IMG_001.jpg"
      }}

      iex> parse("/storage/invalid/path.jpg", "/storage")
      {:error, :invalid_path}
  """
  @spec parse(String.t(), String.t()) :: {:ok, parsed_path()} | {:error, :invalid_path}
  def parse(full_path, storage_root) do
    # Normalize paths (remove trailing slashes)
    normalized_root = String.trim_trailing(storage_root, "/")
    normalized_path = String.trim_trailing(full_path, "/")

    # Get relative path by removing storage root prefix
    relative_path = String.replace_prefix(normalized_path, normalized_root, "")
    relative_path = String.trim_leading(relative_path, "/")

    # Split into path segments
    segments = String.split(relative_path, "/")

    # We need exactly 6 segments: gym, session, group, apparatus, competitor_folder, filename
    case segments do
      [gym_folder, session_folder, group_folder, apparatus, competitor_folder, filename] ->
        with {:ok, gym} <- parse_gym(gym_folder),
             {:ok, session} <- parse_session(session_folder) do
          {:ok,
           %{
             gym: gym,
             session: session,
             group_name: group_folder,
             apparatus: apparatus,
             competitor_folder: competitor_folder,
             filename: filename
           }}
        else
          {:error, _} -> {:error, :invalid_path}
        end

      _ ->
        {:error, :invalid_path}
    end
  end

  @doc """
  Extracts the gym letter from a gym folder name.

  ## Examples

      iex> parse_gym("Gym A")
      {:ok, "A"}

      iex> parse_gym("Gym BC")
      {:ok, "BC"}

      iex> parse_gym("Invalid")
      {:error, :invalid_gym}
  """
  @spec parse_gym(String.t()) :: {:ok, String.t()} | {:error, :invalid_gym}
  def parse_gym("Gym " <> letter) when byte_size(letter) > 0, do: {:ok, letter}
  def parse_gym(_), do: {:error, :invalid_gym}

  @doc """
  Extracts the session identifier from a session folder name.

  ## Examples

      iex> parse_session("Session 1A")
      {:ok, "1A"}

      iex> parse_session("Session 2B")
      {:ok, "2B"}

      iex> parse_session("Invalid")
      {:error, :invalid_session}
  """
  @spec parse_session(String.t()) :: {:ok, String.t()} | {:error, :invalid_session}
  def parse_session("Session " <> id) when byte_size(id) > 0, do: {:ok, id}
  def parse_session(_), do: {:error, :invalid_session}
end
