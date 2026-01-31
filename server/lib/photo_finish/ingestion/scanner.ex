defmodule PhotoFinish.Ingestion.Scanner do
  @moduledoc """
  Scans directories for JPEG files and creates database records.
  """

  require Logger

  @jpeg_extensions ~w(.jpg .jpeg .JPG .JPEG)

  @type file_info :: %{
          path: String.t(),
          filename: String.t(),
          size: non_neg_integer(),
          mtime: integer()
        }

  @type scan_result :: %{
          photos_found: non_neg_integer(),
          photos_new: non_neg_integer(),
          photos_skipped: non_neg_integer(),
          errors: [String.t()]
        }

  @doc """
  Scans a directory recursively for JPEG files.

  Returns {:ok, [file_info]} or {:error, reason}
  """
  @spec scan_directory(String.t()) :: {:ok, [file_info()]} | {:error, :directory_not_found}
  def scan_directory(path) do
    if File.dir?(path) do
      files =
        path
        |> Path.join("**/*")
        |> Path.wildcard()
        |> Enum.filter(&jpeg_file?/1)
        |> Enum.map(&build_file_info/1)
        |> Enum.reject(&is_nil/1)

      {:ok, files}
    else
      {:error, :directory_not_found}
    end
  end

  @doc """
  Creates a signature for duplicate detection.
  """
  @spec file_signature(String.t()) :: {:ok, map()} | {:error, term()}
  def file_signature(path) do
    case File.stat(path) do
      {:ok, stat} ->
        {:ok,
         %{
           filename: Path.basename(path),
           size: stat.size,
           mtime: stat.mtime |> :calendar.datetime_to_gregorian_seconds()
         }}

      {:error, reason} ->
        {:error, reason}
    end
  end

  defp jpeg_file?(path) do
    File.regular?(path) && Path.extname(path) in @jpeg_extensions
  end

  defp build_file_info(path) do
    case file_signature(path) do
      {:ok, sig} ->
        Map.merge(sig, %{path: path})

      {:error, _} ->
        nil
    end
  end
end
