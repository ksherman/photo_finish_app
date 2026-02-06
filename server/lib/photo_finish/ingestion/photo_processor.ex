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
  def perform(%Oban.Job{
        args: %{
          "photo_id" => photo_id,
          "event_id" => event_id,
          "ingestion_path" => ingestion_path,
          "storage_root" => storage_root
        }
      }) do
    thumb_path = thumbnail_path(storage_root, photo_id)
    prev_path = preview_path(storage_root, photo_id)

    with {:ok, _} <- resize_image(ingestion_path, thumb_path, @thumbnail_size),
         {:ok, _} <- resize_image(ingestion_path, prev_path, @preview_size),
         :ok <- mark_ready(photo_id, thumb_path, prev_path) do
      broadcast(event_id, photo_id, :ready)
      :ok
    else
      {:error, reason} ->
        Logger.error("Failed to process photo #{photo_id}: #{inspect(reason)}")
        mark_error(photo_id, inspect(reason))
        {:error, reason}
    end
  end

  defp mark_ready(photo_id, thumb_path, prev_path) do
    Photo
    |> Ash.get!(photo_id)
    |> Ash.update!(%{
      status: :ready,
      thumbnail_path: thumb_path,
      preview_path: prev_path,
      processed_at: DateTime.utc_now()
    })

    :ok
  end

  defp mark_error(photo_id, message) do
    case Ash.get(Photo, photo_id) do
      {:ok, photo} when not is_nil(photo) ->
        Ash.update(photo, %{status: :error, error_message: message})

      _ ->
        :ok
    end
  end

  defp resize_image(input_path, output_path, size) do
    output_path |> Path.dirname() |> File.mkdir_p!()

    with {:ok, image} <- Image.new_from_file(input_path),
         {:ok, resized} <- Operation.thumbnail_image(image, size),
         :ok <- Image.write_to_file(resized, output_path) do
      {:ok, output_path}
    end
  end

  defp broadcast(event_id, photo_id, status) do
    Phoenix.PubSub.broadcast(
      PhotoFinish.PubSub,
      "photos:event:#{event_id}",
      {:photo_status_changed, %{photo_id: photo_id, status: status}}
    )
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
