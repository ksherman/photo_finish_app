defmodule PhotoFinish.Ingestion do
  @moduledoc """
  Context for photo ingestion - scanning directories and processing photos.
  """

  require Logger
  require Ash.Query

  alias PhotoFinish.Ingestion.{Scanner, CompetitorMatcher, PhotoProcessor, PathParser}
  alias PhotoFinish.Events.{Event, EventCompetitor}
  alias PhotoFinish.Photos.Photo

  @chunk_size 5000

  @type scan_result :: %{
          photos_found: non_neg_integer(),
          photos_new: non_neg_integer(),
          photos_skipped: non_neg_integer(),
          errors: [String.t()]
        }

  @doc """
  Scans an event's storage directory for photos.

  Creates photo records and queues processing jobs.
  Photos are created in bulk and Oban jobs are batch-inserted
  in chunks of #{@chunk_size} for efficiency at scale.
  """
  @spec scan_event(String.t()) :: {:ok, scan_result()} | {:error, term()}
  def scan_event(event_id) do
    with {:ok, event} <- load_event(event_id),
         {:ok, files} <- Scanner.scan_directory(event.storage_root) do
      event_competitors = load_event_competitors(event_id)
      existing_paths = load_existing_paths(event_id)

      # Partition into new vs already-ingested files
      {new_files, skipped_count} =
        Enum.reduce(files, {[], 0}, fn file, {new_acc, skip_count} ->
          if MapSet.member?(existing_paths, file.path),
            do: {new_acc, skip_count + 1},
            else: {[file | new_acc], skip_count}
        end)

      Logger.info(
        "Scan found #{length(files)} files: #{length(new_files)} new, #{skipped_count} skipped"
      )

      # Build attrs, bulk-create photos, and bulk-queue Oban jobs in chunks
      {created_count, errors} =
        new_files
        |> Stream.map(&build_photo_attrs(event, &1, event_competitors))
        |> Stream.chunk_every(@chunk_size)
        |> Enum.reduce({0, []}, fn chunk, {created, errs} ->
          {count, chunk_errs} = bulk_create_and_queue(chunk, event)
          Logger.info("Bulk created #{count} photos (#{created + count} total)")
          {created + count, errs ++ chunk_errs}
        end)

      {:ok,
       %{
         photos_found: length(files),
         photos_new: created_count,
         photos_skipped: skipped_count,
         errors: errors
       }}
    end
  end

  defp load_event(event_id) do
    case Ash.get(Event, event_id) do
      {:ok, nil} -> {:error, :event_not_found}
      {:ok, event} -> {:ok, event}
      error -> error
    end
  end

  defp load_event_competitors(event_id) do
    EventCompetitor
    |> Ash.Query.filter(event_id == ^event_id)
    |> Ash.read!()
  end

  defp load_existing_paths(event_id) do
    Photo
    |> Ash.Query.filter(event_id == ^event_id)
    |> Ash.Query.select([:ingestion_path])
    |> Ash.read!()
    |> MapSet.new(& &1.ingestion_path)
  end

  defp build_photo_attrs(event, file, event_competitors) do
    location_info = PathParser.parse(file.path, event.storage_root)

    folder_name =
      case location_info do
        {:ok, info} -> info.competitor_folder
        _ -> Path.basename(Path.dirname(file.path))
      end

    event_competitor_id =
      case match_event_competitor(folder_name, event_competitors) do
        {:ok, ec} -> ec.id
        :no_match -> nil
      end

    base_attrs = %{
      event_id: event.id,
      event_competitor_id: event_competitor_id,
      ingestion_path: file.path,
      filename: file.filename,
      original_filename: file.filename,
      file_size_bytes: file.size,
      status: :discovered
    }

    case location_info do
      {:ok, info} ->
        Map.merge(base_attrs, %{
          gym: info.gym,
          session: info.session,
          group_name: info.group_name,
          apparatus: info.apparatus,
          source_folder: info.competitor_folder
        })

      _ ->
        base_attrs
    end
  end

  defp bulk_create_and_queue(attrs_chunk, event) do
    result =
      Ash.bulk_create(attrs_chunk, Photo, :create,
        return_records?: true,
        return_errors?: true,
        stop_on_error?: false
      )

    records = result.records || []

    # Bulk-insert Oban jobs with all data needed to skip the DB read
    if records != [] do
      records
      |> Enum.map(fn photo ->
        PhotoProcessor.new(%{
          photo_id: photo.id,
          event_id: event.id,
          ingestion_path: photo.ingestion_path,
          storage_root: event.storage_root
        })
      end)
      |> Oban.insert_all()
    end

    errors =
      case result.errors do
        errs when errs in [nil, []] ->
          []

        errs ->
          mapped = Enum.map(errs, &inspect/1)

          Logger.warning(
            "Batch had #{length(mapped)} error(s) out of #{length(attrs_chunk)} records: #{Enum.take(mapped, 3) |> Enum.join(", ")}"
          )

          mapped
      end

    {length(records), errors}
  end

  defp match_event_competitor(folder_name, event_competitors) do
    case CompetitorMatcher.extract_competitor_number(folder_name) do
      {:ok, number} ->
        CompetitorMatcher.find_event_competitor(event_competitors, number)

      :no_match ->
        :no_match
    end
  end
end
