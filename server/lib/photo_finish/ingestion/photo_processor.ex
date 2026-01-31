defmodule PhotoFinish.Ingestion.PhotoProcessor do
  @moduledoc """
  Oban worker that generates thumbnails and previews for photos.
  """

  use Oban.Worker, queue: :media, max_attempts: 3

  require Logger

  @thumbnail_size 320
  @preview_size 1280

  @impl Oban.Worker
  def perform(%Oban.Job{args: %{"photo_id" => photo_id}}) do
    Logger.info("Processing photo #{photo_id}")

    # TODO: Implement in Task 7 after Vix is added
    # For now, just mark as ready (stub)
    {:ok, :processed}
  end

  @doc """
  Builds the output path for a processed image.
  """
  @spec build_output_path(String.t(), String.t(), String.t()) :: String.t()
  def build_output_path(root, event_slug, photo_id) do
    Path.join([root, event_slug, "#{photo_id}.jpg"])
  end

  # Getters for sizes (useful for testing)
  def thumbnail_size, do: @thumbnail_size
  def preview_size, do: @preview_size
end
