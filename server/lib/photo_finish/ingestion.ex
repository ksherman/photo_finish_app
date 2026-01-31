defmodule PhotoFinish.Ingestion do
  @moduledoc """
  Context for photo ingestion - scanning directories and processing photos.
  """

  require Logger

  alias PhotoFinish.Ingestion.{Scanner, CompetitorMatcher, PhotoProcessor}
  alias PhotoFinish.Events.{Event, Competitor}
  alias PhotoFinish.Photos.Photo

  @type scan_result :: %{
          photos_found: non_neg_integer(),
          photos_new: non_neg_integer(),
          photos_skipped: non_neg_integer(),
          errors: [String.t()]
        }

  @doc """
  Scans an event's storage directory for photos.

  Creates photo records and queues processing jobs.
  """
  @spec scan_event(String.t()) :: {:ok, scan_result()} | {:error, term()}
  def scan_event(event_id) do
    with {:ok, event} <- load_event(event_id),
         {:ok, files} <- Scanner.scan_directory(event.storage_directory) do
      competitors = load_competitors(event_id)

      result =
        Enum.reduce(
          files,
          %{photos_found: 0, photos_new: 0, photos_skipped: 0, errors: []},
          fn file, acc ->
            acc = %{acc | photos_found: acc.photos_found + 1}

            case process_file(event, file, competitors) do
              {:ok, :created} ->
                %{acc | photos_new: acc.photos_new + 1}

              {:ok, :skipped} ->
                %{acc | photos_skipped: acc.photos_skipped + 1}

              {:error, reason} ->
                %{acc | errors: [reason | acc.errors]}
            end
          end
        )

      {:ok, result}
    end
  end

  defp load_event(event_id) do
    case Ash.get(Event, event_id) do
      {:ok, nil} -> {:error, :event_not_found}
      {:ok, event} -> {:ok, event}
      error -> error
    end
  end

  defp load_competitors(event_id) do
    Ash.read!(Competitor)
    |> Enum.filter(&(&1.event_id == event_id))
  end

  defp process_file(event, file, competitors) do
    # Check if photo already exists (by signature)
    if photo_exists?(event.id, file) do
      {:ok, :skipped}
    else
      # Extract folder name for competitor matching
      folder_name = Path.basename(Path.dirname(file.path))
      competitor = match_competitor(folder_name, competitors)
      create_photo(event, competitor, file)
    end
  end

  defp photo_exists?(event_id, file) do
    Ash.read!(Photo)
    |> Enum.any?(fn p ->
      p.event_id == event_id &&
        p.filename == file.filename &&
        p.file_size_bytes == file.size
    end)
  end

  defp match_competitor(folder_name, competitors) do
    case CompetitorMatcher.extract_competitor_number(folder_name) do
      {:ok, number} ->
        CompetitorMatcher.find_competitor(competitors, number)

      :no_match ->
        :no_match
    end
  end

  defp create_photo(event, competitor, file) do
    competitor_id =
      case competitor do
        {:ok, c} -> c.id
        :no_match -> nil
      end

    case Ash.create(Photo, %{
           event_id: event.id,
           competitor_id: competitor_id,
           ingestion_path: file.path,
           filename: file.filename,
           original_filename: file.filename,
           file_size_bytes: file.size,
           status: :discovered
         }) do
      {:ok, photo} ->
        queue_processing(photo)
        {:ok, :created}

      {:error, reason} ->
        {:error, reason}
    end
  end

  defp queue_processing(photo) do
    %{photo_id: photo.id}
    |> PhotoProcessor.new()
    |> Oban.insert()
  end
end
