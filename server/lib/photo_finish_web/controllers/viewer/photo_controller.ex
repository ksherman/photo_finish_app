defmodule PhotoFinishWeb.Viewer.PhotoController do
  @moduledoc """
  Serves photo thumbnails and previews for the public viewer.
  """
  use PhotoFinishWeb, :controller

  alias PhotoFinish.Photos.Photo

  def thumbnail(conn, %{"id" => id}) do
    case Ash.get(Photo, id) do
      {:ok, photo} when not is_nil(photo) and not is_nil(photo.thumbnail_path) ->
        serve_file(conn, photo.thumbnail_path)

      _ ->
        send_resp(conn, 404, "Not found")
    end
  end

  def preview(conn, %{"id" => id}) do
    case Ash.get(Photo, id) do
      {:ok, photo} when not is_nil(photo) and not is_nil(photo.preview_path) ->
        serve_file(conn, photo.preview_path)

      _ ->
        send_resp(conn, 404, "Not found")
    end
  end

  defp serve_file(conn, path) do
    if File.exists?(path) do
      conn
      |> put_resp_content_type("image/jpeg")
      |> put_resp_header("cache-control", "public, max-age=31536000")
      |> send_file(200, path)
    else
      send_resp(conn, 404, "File not found")
    end
  end
end
