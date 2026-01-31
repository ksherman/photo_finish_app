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
         {:ok, photo} <- generate_thumbnail(photo),
         {:ok, photo} <- generate_preview(photo),
         {:ok, _photo} <- mark_ready(photo) do
      Logger.info("Successfully processed photo #{photo_id}")
      :ok
    else
      {:error, reason} ->
        Logger.error("Failed to process photo #{photo_id}: #{inspect(reason)}")
        mark_error(photo_id, inspect(reason))
        {:error, reason}
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
    output_path =
      build_output_path(
        thumbnail_root(),
        photo.event.slug,
        photo.id
      )

    case resize_image(photo.ingestion_path, output_path, @thumbnail_size) do
      {:ok, _} ->
        Ash.update(photo, %{thumbnail_path: output_path})

      {:error, reason} ->
        {:error, {:thumbnail_failed, reason}}
    end
  end

  defp generate_preview(photo) do
    output_path =
      build_output_path(
        preview_root(),
        photo.event.slug,
        photo.id
      )

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
         {:ok, resized} <- Operation.thumbnail_image(image, size) do
      Image.write_to_file(resized, output_path)
    end
  end

  @doc """
  Builds the output path for a processed image.
  """
  @spec build_output_path(String.t(), String.t(), String.t()) :: String.t()
  def build_output_path(root, event_slug, photo_id) do
    Path.join([root, event_slug, "#{photo_id}.jpg"])
  end

  defp thumbnail_root do
    Application.get_env(:photo_finish, :thumbnail_root, "/tmp/thumbnails")
  end

  defp preview_root do
    Application.get_env(:photo_finish, :preview_root, "/tmp/previews")
  end

  def thumbnail_size, do: @thumbnail_size
  def preview_size, do: @preview_size
end
