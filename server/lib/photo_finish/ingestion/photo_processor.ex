defmodule PhotoFinish.Ingestion.PhotoProcessor do
  @moduledoc """
  Oban worker that generates thumbnails and previews for photos.
  """

  use Oban.Worker, queue: :media, max_attempts: 3

  require Logger

  alias PhotoFinish.Photos.Photo
  alias Vix.Vips.Image
  alias Vix.Vips.Operation

  @thumbnail_size 320
  @preview_size 1280

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"photo_id" => photo_id}}) do
    with {:ok, photo} <- load_photo(photo_id),
         {:ok, photo} <- mark_processing(photo),
         :ok <- broadcast_progress(photo, :processing),
         {:ok, photo} <- generate_thumbnail(photo),
         {:ok, photo} <- generate_preview(photo),
         {:ok, photo} <- mark_ready(photo) do
      Logger.info("Successfully processed photo #{photo_id}")
      broadcast_progress(photo, :ready)
      :ok
    else
      {:error, reason} ->
        Logger.error("Failed to process photo #{photo_id}: #{inspect(reason)}")
        mark_error(photo_id, inspect(reason))
        broadcast_error(photo_id)
        {:error, reason}
    end
  end

  defp broadcast_progress(photo, status) do
    Phoenix.PubSub.broadcast(
      PhotoFinish.PubSub,
      "photos:event:#{photo.event_id}",
      {:photo_status_changed, %{photo_id: photo.id, status: status}}
    )
    :ok
  end

  defp broadcast_error(photo_id) do
    case Ash.get(Photo, photo_id) do
      {:ok, photo} when not is_nil(photo) ->
        Phoenix.PubSub.broadcast(
          PhotoFinish.PubSub,
          "photos:event:#{photo.event_id}",
          {:photo_status_changed, %{photo_id: photo.id, status: :error}}
        )
      _ ->
        :ok
    end
  end

  defp load_photo(photo_id) do
    case Ash.get(Photo, photo_id, load: [:event]) do
      {:ok, nil} -> {:error, :photo_not_found}
      {:ok, photo} -> {:ok, photo}
      error -> error
    end
  end

  defp mark_processing(photo) do
    Ash.update(photo, %{status: :processing})
  end

  defp mark_ready(photo) do
    Ash.update(photo, %{
      status: :ready,
      processed_at: DateTime.utc_now()
    })
  end

  defp mark_error(photo_id, message) do
    case Ash.get(Photo, photo_id) do
      {:ok, photo} when not is_nil(photo) ->
        Ash.update(photo, %{status: :error, error_message: message})

      _ ->
        :ok
    end
  end

  defp generate_thumbnail(photo) do
    output_path = thumbnail_path(photo.event.storage_root, photo.id)

    case resize_image(photo.ingestion_path, output_path, @thumbnail_size) do
      {:ok, _} ->
        Ash.update(photo, %{thumbnail_path: output_path})

      {:error, reason} ->
        {:error, {:thumbnail_failed, reason}}
    end
  end

  defp generate_preview(photo) do
    output_path = preview_path(photo.event.storage_root, photo.id)

    case resize_image(photo.ingestion_path, output_path, @preview_size) do
      {:ok, _} ->
        Ash.update(photo, %{preview_path: output_path})

      {:error, reason} ->
        {:error, {:preview_failed, reason}}
    end
  end

  defp resize_image(input_path, output_path, size) do
    # Ensure output directory exists
    output_path |> Path.dirname() |> File.mkdir_p!()

    with {:ok, image} <- Image.new_from_file(input_path),
         {:ok, resized} <- Operation.thumbnail_image(image, size),
         :ok <- Image.write_to_file(resized, output_path) do
      {:ok, output_path}
    end
  end

  @doc """
  Builds the thumbnail path for a photo within the event's storage root.
  """
  @spec thumbnail_path(String.t(), String.t()) :: String.t()
  def thumbnail_path(storage_root, photo_id) do
    Path.join([storage_root, "_thumbnails", "#{photo_id}.jpg"])
  end

  @doc """
  Builds the preview path for a photo within the event's storage root.
  """
  @spec preview_path(String.t(), String.t()) :: String.t()
  def preview_path(storage_root, photo_id) do
    Path.join([storage_root, "_previews", "#{photo_id}.jpg"])
  end

  def thumbnail_size, do: @thumbnail_size
  def preview_size, do: @preview_size
end
