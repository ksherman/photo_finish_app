# Ingestion Pipeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a manual scan system that discovers photos on NAS, creates database records, and generates thumbnails/previews in background.

**Architecture:** Admin clicks "Scan Now" → Scanner walks event's storage_directory → Creates hierarchy_nodes and photo records → Queues Oban jobs for thumbnail/preview generation.

**Tech Stack:** Elixir/Phoenix, Ash Framework, Oban for background jobs, Vix (libvips) for image processing.

**Design Doc:** `docs/plans/2025-01-31-ingestion-pipeline-design.md`

---

## Task 1: Add Oban Media Queue

**Files:**
- Modify: `server/config/config.exs`

**Step 1: Add media queue to Oban config**

In `config/config.exs`, update the Oban config to add the media queue:

```elixir
config :photo_finish, Oban,
  engine: Oban.Engines.Basic,
  notifier: Oban.Notifiers.Postgres,
  queues: [default: 10, media: 4],
  repo: PhotoFinish.Repo,
  plugins: [{Oban.Plugins.Cron, []}]
```

**Step 2: Verify config loads**

Run: `cd server && mix compile`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add config/config.exs
git commit -m "feat(ingestion): add Oban media queue for image processing"
```

---

## Task 2: Create FolderParser Module

**Files:**
- Create: `server/lib/photo_finish/ingestion/folder_parser.ex`
- Create: `server/test/photo_finish/ingestion/folder_parser_test.exs`

**Step 1: Write the test file**

```elixir
defmodule PhotoFinish.Ingestion.FolderParserTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.FolderParser

  describe "parse_path/2" do
    test "parses folder path relative to storage root" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/events/valentines-2025/Gym A/Session 3/Group 1A/Beam/1022 Kevin S"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == [
               {1, "Gym A"},
               {2, "Session 3"},
               {3, "Group 1A"},
               {4, "Beam"},
               {5, "1022 Kevin S"}
             ]
    end

    test "handles trailing slashes" do
      storage_root = "/NAS/events/valentines-2025/"
      full_path = "/NAS/events/valentines-2025/Gym A/Session 1/"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == [
               {1, "Gym A"},
               {2, "Session 1"}
             ]
    end

    test "returns empty list for root path" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/events/valentines-2025"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == []
    end

    test "returns error if path not under root" do
      storage_root = "/NAS/events/valentines-2025"
      full_path = "/NAS/other/path"

      result = FolderParser.parse_path(full_path, storage_root)

      assert result == {:error, :path_not_under_root}
    end
  end

  describe "slugify/1" do
    test "converts name to slug" do
      assert FolderParser.slugify("Gym A") == "gym-a"
      assert FolderParser.slugify("Session 3") == "session-3"
      assert FolderParser.slugify("1022 Kevin S") == "1022-kevin-s"
    end
  end
end
```

**Step 2: Run test to verify it fails**

Run: `cd server && mix test test/photo_finish/ingestion/folder_parser_test.exs`
Expected: FAIL with "module FolderParser is not available"

**Step 3: Write the implementation**

```elixir
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
  @spec parse_path(String.t(), String.t()) :: [{pos_integer(), String.t()}] | {:error, :path_not_under_root}
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
```

**Step 4: Run test to verify it passes**

Run: `cd server && mix test test/photo_finish/ingestion/folder_parser_test.exs`
Expected: All tests pass

**Step 5: Commit**

```bash
git add lib/photo_finish/ingestion/folder_parser.ex test/photo_finish/ingestion/folder_parser_test.exs
git commit -m "feat(ingestion): add FolderParser module"
```

---

## Task 3: Create CompetitorMatcher Module

**Files:**
- Create: `server/lib/photo_finish/ingestion/competitor_matcher.ex`
- Create: `server/test/photo_finish/ingestion/competitor_matcher_test.exs`

**Step 1: Write the test file**

```elixir
defmodule PhotoFinish.Ingestion.CompetitorMatcherTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.CompetitorMatcher

  describe "extract_competitor_number/1" do
    test "extracts number from folder name" do
      assert CompetitorMatcher.extract_competitor_number("1022 Kevin S") == {:ok, "1022"}
      assert CompetitorMatcher.extract_competitor_number("123 Jane Doe") == {:ok, "123"}
      assert CompetitorMatcher.extract_competitor_number("9999 A") == {:ok, "9999"}
    end

    test "returns no_match for non-matching patterns" do
      assert CompetitorMatcher.extract_competitor_number("Gymnast 01") == :no_match
      assert CompetitorMatcher.extract_competitor_number("Floor") == :no_match
      assert CompetitorMatcher.extract_competitor_number("Group 1A") == :no_match
    end

    test "handles edge cases" do
      assert CompetitorMatcher.extract_competitor_number("1022") == :no_match
      assert CompetitorMatcher.extract_competitor_number("") == :no_match
      assert CompetitorMatcher.extract_competitor_number("  1022 Kevin") == {:ok, "1022"}
    end
  end
end
```

**Step 2: Run test to verify it fails**

Run: `cd server && mix test test/photo_finish/ingestion/competitor_matcher_test.exs`
Expected: FAIL with "module CompetitorMatcher is not available"

**Step 3: Write the implementation**

```elixir
defmodule PhotoFinish.Ingestion.CompetitorMatcher do
  @moduledoc """
  Matches folder names to competitors in the roster.
  """

  # Pattern: starts with digits, followed by space and name
  @competitor_pattern ~r/^\s*(\d+)\s+\S/

  @doc """
  Extracts competitor number from a folder name.

  Expects format: "{number} {name}" (e.g., "1022 Kevin S")

  Returns {:ok, number} or :no_match
  """
  @spec extract_competitor_number(String.t()) :: {:ok, String.t()} | :no_match
  def extract_competitor_number(folder_name) do
    case Regex.run(@competitor_pattern, folder_name) do
      [_, number] -> {:ok, number}
      _ -> :no_match
    end
  end

  @doc """
  Finds a competitor by number in the given list.

  Returns {:ok, competitor} or :no_match
  """
  @spec find_competitor([map()], String.t()) :: {:ok, map()} | :no_match
  def find_competitor(competitors, competitor_number) do
    case Enum.find(competitors, &(&1.competitor_number == competitor_number)) do
      nil -> :no_match
      competitor -> {:ok, competitor}
    end
  end
end
```

**Step 4: Run test to verify it passes**

Run: `cd server && mix test test/photo_finish/ingestion/competitor_matcher_test.exs`
Expected: All tests pass

**Step 5: Commit**

```bash
git add lib/photo_finish/ingestion/competitor_matcher.ex test/photo_finish/ingestion/competitor_matcher_test.exs
git commit -m "feat(ingestion): add CompetitorMatcher module"
```

---

## Task 4: Create PhotoProcessor Oban Worker

**Files:**
- Create: `server/lib/photo_finish/ingestion/photo_processor.ex`
- Create: `server/test/photo_finish/ingestion/photo_processor_test.exs`

**Step 1: Write the test file**

```elixir
defmodule PhotoFinish.Ingestion.PhotoProcessorTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Ingestion.PhotoProcessor

  describe "new/1" do
    test "creates a valid Oban job" do
      job = PhotoProcessor.new(%{photo_id: "abc-123"})

      assert job.args == %{photo_id: "abc-123"}
      assert job.queue == "media"
    end
  end

  describe "build_output_path/3" do
    test "builds thumbnail path" do
      result = PhotoProcessor.build_output_path(
        "/NAS/thumbnails",
        "valentines-2025",
        "photo-uuid-123"
      )

      assert result == "/NAS/thumbnails/valentines-2025/photo-uuid-123.jpg"
    end
  end
end
```

**Step 2: Run test to verify it fails**

Run: `cd server && mix test test/photo_finish/ingestion/photo_processor_test.exs`
Expected: FAIL with "module PhotoProcessor is not available"

**Step 3: Write the implementation**

```elixir
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
```

**Step 4: Run test to verify it passes**

Run: `cd server && mix test test/photo_finish/ingestion/photo_processor_test.exs`
Expected: All tests pass

**Step 5: Commit**

```bash
git add lib/photo_finish/ingestion/photo_processor.ex test/photo_finish/ingestion/photo_processor_test.exs
git commit -m "feat(ingestion): add PhotoProcessor Oban worker (stub)"
```

---

## Task 5: Create Scanner Module

**Files:**
- Create: `server/lib/photo_finish/ingestion/scanner.ex`
- Create: `server/test/photo_finish/ingestion/scanner_test.exs`

**Step 1: Write the test file**

```elixir
defmodule PhotoFinish.Ingestion.ScannerTest do
  use PhotoFinish.DataCase, async: false

  alias PhotoFinish.Ingestion.Scanner
  alias PhotoFinish.Events

  describe "scan_directory/2" do
    test "finds JPEG files recursively" do
      # Create a temp directory structure
      tmp_dir = System.tmp_dir!() |> Path.join("scanner_test_#{:rand.uniform(10000)}")
      File.mkdir_p!(Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S"]))

      # Create test files
      jpeg_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "IMG_001.jpg"])
      File.write!(jpeg_path, "fake jpeg content")

      # Also create a non-JPEG to ensure it's filtered
      txt_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "notes.txt"])
      File.write!(txt_path, "some notes")

      try do
        {:ok, files} = Scanner.scan_directory(tmp_dir)

        assert length(files) == 1
        assert hd(files).path == jpeg_path
        assert hd(files).filename == "IMG_001.jpg"
      after
        File.rm_rf!(tmp_dir)
      end
    end

    test "returns error for non-existent directory" do
      result = Scanner.scan_directory("/non/existent/path")

      assert result == {:error, :directory_not_found}
    end
  end

  describe "file_signature/1" do
    test "creates signature from file stat" do
      tmp_file = System.tmp_dir!() |> Path.join("test_file_#{:rand.uniform(10000)}.jpg")
      File.write!(tmp_file, "test content")

      try do
        {:ok, sig} = Scanner.file_signature(tmp_file)

        assert sig.filename == Path.basename(tmp_file)
        assert sig.size == 12  # "test content" is 12 bytes
        assert is_integer(sig.mtime)
      after
        File.rm!(tmp_file)
      end
    end
  end
end
```

**Step 2: Run test to verify it fails**

Run: `cd server && mix test test/photo_finish/ingestion/scanner_test.exs`
Expected: FAIL with "module Scanner is not available"

**Step 3: Write the implementation**

```elixir
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
        {:ok, %{
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
```

**Step 4: Run test to verify it passes**

Run: `cd server && mix test test/photo_finish/ingestion/scanner_test.exs`
Expected: All tests pass

**Step 5: Commit**

```bash
git add lib/photo_finish/ingestion/scanner.ex test/photo_finish/ingestion/scanner_test.exs
git commit -m "feat(ingestion): add Scanner module for directory walking"
```

---

## Task 6: Create Ingestion Context with scan_event/1

**Files:**
- Create: `server/lib/photo_finish/ingestion.ex`
- Create: `server/test/photo_finish/ingestion_test.exs`
- Modify: `server/test/support/data_case.ex` (if needed for test setup)

**Step 1: Write the test file**

```elixir
defmodule PhotoFinish.IngestionTest do
  use PhotoFinish.DataCase, async: false

  alias PhotoFinish.Ingestion
  alias PhotoFinish.Events
  alias PhotoFinish.Photos

  describe "scan_event/1" do
    setup do
      # Create a temp directory structure
      tmp_dir = System.tmp_dir!() |> Path.join("ingestion_test_#{:rand.uniform(100000)}")
      File.mkdir_p!(Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S"]))

      # Create test JPEG
      jpeg_path = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S", "IMG_001.jpg"])
      File.write!(jpeg_path, "fake jpeg content for testing")

      # Create an event with storage_directory pointing to tmp_dir
      {:ok, event} = Ash.create(PhotoFinish.Events.Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_directory: tmp_dir
      })

      on_exit(fn -> File.rm_rf!(tmp_dir) end)

      %{event: event, tmp_dir: tmp_dir}
    end

    test "creates hierarchy nodes from folder structure", %{event: event} do
      {:ok, result} = Ingestion.scan_event(event.id)

      assert result.photos_found == 1
      assert result.photos_new == 1

      # Verify hierarchy nodes were created
      nodes = Ash.read!(PhotoFinish.Events.HierarchyNode)
      |> Enum.filter(&(&1.event_id == event.id))

      assert length(nodes) == 3  # Gym A, Session 1, 1022 Kevin S

      gym_node = Enum.find(nodes, &(&1.name == "Gym A"))
      assert gym_node.level_number == 1

      session_node = Enum.find(nodes, &(&1.name == "Session 1"))
      assert session_node.level_number == 2
      assert session_node.parent_id == gym_node.id
    end

    test "creates photo records", %{event: event} do
      {:ok, _result} = Ingestion.scan_event(event.id)

      photos = Ash.read!(PhotoFinish.Photos.Photo)
      |> Enum.filter(&(&1.event_id == event.id))

      assert length(photos) == 1
      photo = hd(photos)
      assert photo.filename == "IMG_001.jpg"
      assert photo.status == :discovered
    end

    test "is idempotent - skips existing photos", %{event: event} do
      {:ok, result1} = Ingestion.scan_event(event.id)
      assert result1.photos_new == 1

      {:ok, result2} = Ingestion.scan_event(event.id)
      assert result2.photos_new == 0
      assert result2.photos_skipped == 1
    end

    test "returns error for missing storage directory" do
      {:ok, event} = Ash.create(PhotoFinish.Events.Event, %{
        name: "No Storage",
        slug: "no-storage",
        storage_directory: "/non/existent/path"
      })

      result = Ingestion.scan_event(event.id)

      assert {:error, :directory_not_found} = result
    end
  end
end
```

**Step 2: Run test to verify it fails**

Run: `cd server && mix test test/photo_finish/ingestion_test.exs`
Expected: FAIL with "module Ingestion is not available"

**Step 3: Write the implementation**

```elixir
defmodule PhotoFinish.Ingestion do
  @moduledoc """
  Context for photo ingestion - scanning directories and processing photos.
  """

  require Logger

  alias PhotoFinish.Ingestion.{Scanner, FolderParser, CompetitorMatcher, PhotoProcessor}
  alias PhotoFinish.Events
  alias PhotoFinish.Events.{Event, HierarchyNode, Competitor}
  alias PhotoFinish.Photos.Photo

  @type scan_result :: %{
          photos_found: non_neg_integer(),
          photos_new: non_neg_integer(),
          photos_skipped: non_neg_integer(),
          errors: [String.t()]
        }

  @doc """
  Scans an event's storage directory for photos.

  Creates hierarchy nodes to mirror folder structure,
  creates photo records, and queues processing jobs.
  """
  @spec scan_event(String.t()) :: {:ok, scan_result()} | {:error, term()}
  def scan_event(event_id) do
    with {:ok, event} <- load_event(event_id),
         {:ok, files} <- Scanner.scan_directory(event.storage_directory) do

      competitors = load_competitors(event_id)

      result =
        Enum.reduce(files, %{photos_found: 0, photos_new: 0, photos_skipped: 0, errors: []}, fn file, acc ->
          acc = %{acc | photos_found: acc.photos_found + 1}

          case process_file(event, file, competitors) do
            {:ok, :created} ->
              %{acc | photos_new: acc.photos_new + 1}

            {:ok, :skipped} ->
              %{acc | photos_skipped: acc.photos_skipped + 1}

            {:error, reason} ->
              %{acc | errors: [reason | acc.errors]}
          end
        end)

      {:ok, result}
    end
  end

  defp load_event(event_id) do
    case Ash.get(Event, event_id, load: [:hierarchy_levels]) do
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
      with {:ok, node} <- ensure_hierarchy_nodes(event, file.path),
           {:ok, competitor} <- match_competitor(node.name, competitors),
           {:ok, photo} <- create_photo(event, node, competitor, file) do
        queue_processing(photo)
        {:ok, :created}
      end
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

  defp ensure_hierarchy_nodes(event, file_path) do
    folder_path = Path.dirname(file_path)

    case FolderParser.parse_path(folder_path, event.storage_directory) do
      {:error, reason} ->
        {:error, reason}

      levels ->
        # Create or find nodes for each level
        {_parent, leaf_node} =
          Enum.reduce(levels, {nil, nil}, fn {level_num, name}, {parent_id, _} ->
            node = find_or_create_node(event.id, parent_id, level_num, name)
            {node.id, node}
          end)

        {:ok, leaf_node}
    end
  end

  defp find_or_create_node(event_id, parent_id, level_number, name) do
    slug = FolderParser.slugify(name)

    existing =
      Ash.read!(HierarchyNode)
      |> Enum.find(fn n ->
        n.event_id == event_id &&
          n.parent_id == parent_id &&
          n.name == name
      end)

    case existing do
      nil ->
        {:ok, node} = Ash.create(HierarchyNode, %{
          event_id: event_id,
          parent_id: parent_id,
          level_number: level_number,
          name: name,
          slug: slug
        })
        node

      node ->
        node
    end
  end

  defp match_competitor(folder_name, competitors) do
    case CompetitorMatcher.extract_competitor_number(folder_name) do
      {:ok, number} ->
        CompetitorMatcher.find_competitor(competitors, number)

      :no_match ->
        :no_match
    end
  end

  defp create_photo(event, node, competitor, file) do
    competitor_id = case competitor do
      {:ok, c} -> c.id
      :no_match -> nil
    end

    Ash.create(Photo, %{
      event_id: event.id,
      node_id: node.id,
      competitor_id: competitor_id,
      ingestion_path: file.path,
      filename: file.filename,
      original_filename: file.filename,
      file_size_bytes: file.size,
      status: :discovered
    })
  end

  defp queue_processing(photo) do
    %{photo_id: photo.id}
    |> PhotoProcessor.new()
    |> Oban.insert()
  end
end
```

**Step 4: Run test to verify it passes**

Run: `cd server && mix test test/photo_finish/ingestion_test.exs`
Expected: All tests pass

**Step 5: Commit**

```bash
git add lib/photo_finish/ingestion.ex test/photo_finish/ingestion_test.exs
git commit -m "feat(ingestion): add Ingestion context with scan_event/1"
```

---

## Task 7: Add Vix and Implement PhotoProcessor

**Files:**
- Modify: `server/mix.exs`
- Modify: `server/lib/photo_finish/ingestion/photo_processor.ex`
- Modify: `server/test/photo_finish/ingestion/photo_processor_test.exs`

**Step 1: Add vix dependency**

In `mix.exs`, add to deps:

```elixir
{:vix, "~> 0.23"}
```

**Step 2: Install dependency**

Run: `cd server && mix deps.get`
Expected: Vix downloads successfully

**Step 3: Update PhotoProcessor with real implementation**

```elixir
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
         {:ok, _} <- generate_thumbnail(photo),
         {:ok, _} <- generate_preview(photo),
         {:ok, _} <- mark_ready(photo) do
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
    output_path = build_output_path(
      thumbnail_root(),
      photo.event.slug,
      photo.id
    )

    resize_image(photo.ingestion_path, output_path, @thumbnail_size)

    Ash.update(photo, %{thumbnail_path: output_path})
  end

  defp generate_preview(photo) do
    output_path = build_output_path(
      preview_root(),
      photo.event.slug,
      photo.id
    )

    resize_image(photo.ingestion_path, output_path, @preview_size)

    Ash.update(photo, %{preview_path: output_path})
  end

  defp resize_image(input_path, output_path, size) do
    # Ensure output directory exists
    output_path |> Path.dirname() |> File.mkdir_p!()

    {:ok, image} = Image.new_from_file(input_path)

    # Resize to fit within size x size, maintaining aspect ratio
    {:ok, resized} = Operation.thumbnail_image(image, size)

    :ok = Image.write_to_file(resized, output_path)

    {:ok, output_path}
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
```

**Step 4: Run all ingestion tests**

Run: `cd server && mix test test/photo_finish/ingestion`
Expected: All tests pass

**Step 5: Commit**

```bash
git add mix.exs mix.lock lib/photo_finish/ingestion/photo_processor.ex
git commit -m "feat(ingestion): implement PhotoProcessor with Vix image resizing"
```

---

## Task 8: Add Scan UI to Admin Event Page

**Files:**
- Modify: `server/lib/photo_finish_web/live/admin/event_live/show.ex`

**Step 1: Add ingestion section to event show page**

Add to the render function, in the overview section, an ingestion panel with scan button:

```elixir
# In the overview content section, add:
<div class="bg-white rounded-lg border border-gray-200 p-4">
  <h3 class="text-sm font-semibold text-gray-700 mb-3">Ingestion</h3>

  <div class="space-y-2 text-sm">
    <div class="flex justify-between">
      <span class="text-gray-500">Storage Path</span>
      <code class="text-xs bg-gray-100 px-2 py-1 rounded">
        {@event.storage_directory || "Not configured"}
      </code>
    </div>

    <div class="flex justify-between">
      <span class="text-gray-500">Photos</span>
      <span>
        <span class="text-green-600 font-medium">{@photo_counts.ready}</span> ready,
        <span class="text-blue-600">{@photo_counts.processing}</span> processing,
        <span class="text-red-600">{@photo_counts.error}</span> errors
      </span>
    </div>
  </div>

  <div class="mt-4 pt-4 border-t border-gray-100">
    <.button
      phx-click="scan_now"
      disabled={is_nil(@event.storage_directory)}
      size="small"
      variant="primary"
    >
      <.icon name="hero-magnifying-glass" class="w-4 h-4 mr-1" />
      Scan Now
    </.button>
  </div>
</div>
```

**Step 2: Add handle_event for scan**

```elixir
@impl true
def handle_event("scan_now", _, socket) do
  case PhotoFinish.Ingestion.scan_event(socket.assigns.event.id) do
    {:ok, result} ->
      socket =
        socket
        |> put_flash(:info, "Scan complete. Found #{result.photos_new} new photos, #{result.photos_skipped} skipped.")
        |> assign(:photo_counts, load_photo_counts(socket.assigns.event.id))

      {:noreply, socket}

    {:error, :directory_not_found} ->
      {:noreply, put_flash(socket, :error, "Storage directory not found.")}

    {:error, reason} ->
      {:noreply, put_flash(socket, :error, "Scan failed: #{inspect(reason)}")}
  end
end
```

**Step 3: Add photo_counts to mount**

```elixir
defp load_photo_counts(event_id) do
  photos = Ash.read!(PhotoFinish.Photos.Photo)
  |> Enum.filter(&(&1.event_id == event_id))

  %{
    ready: Enum.count(photos, &(&1.status == :ready)),
    processing: Enum.count(photos, &(&1.status in [:discovered, :processing])),
    error: Enum.count(photos, &(&1.status == :error))
  }
end
```

**Step 4: Test manually**

Run: `cd server && mix phx.server`
Navigate to an event page with a storage_directory set
Click "Scan Now"
Expected: Flash message shows scan results

**Step 5: Commit**

```bash
git add lib/photo_finish_web/live/admin/event_live/show.ex
git commit -m "feat(admin): add scan button to event page"
```

---

## Task 9: Add Runtime Config for Paths

**Files:**
- Modify: `server/config/runtime.exs`

**Step 1: Add ingestion config to runtime.exs**

```elixir
# Photo storage paths
config :photo_finish,
  photo_root: System.get_env("PHOTO_ROOT") || "/tmp/photos/originals",
  preview_root: System.get_env("PREVIEW_ROOT") || "/tmp/photos/previews",
  thumbnail_root: System.get_env("THUMBNAIL_ROOT") || "/tmp/photos/thumbnails",
  thumbnail_size: String.to_integer(System.get_env("THUMBNAIL_SIZE", "320")),
  preview_size: String.to_integer(System.get_env("PREVIEW_SIZE", "1280"))
```

**Step 2: Verify config loads**

Run: `cd server && iex -S mix`
Run: `Application.get_env(:photo_finish, :thumbnail_root)`
Expected: Returns "/tmp/photos/thumbnails" (or env override)

**Step 3: Commit**

```bash
git add config/runtime.exs
git commit -m "feat(config): add runtime config for photo storage paths"
```

---

## Task 10: Final Integration Test

**Files:**
- Create: `server/test/photo_finish/ingestion_integration_test.exs`

**Step 1: Write integration test**

```elixir
defmodule PhotoFinish.IngestionIntegrationTest do
  use PhotoFinish.DataCase, async: false
  use Oban.Testing, repo: PhotoFinish.Repo

  alias PhotoFinish.Ingestion
  alias PhotoFinish.Ingestion.PhotoProcessor

  describe "full ingestion flow" do
    setup do
      # Create temp directory with test structure
      tmp_dir = System.tmp_dir!() |> Path.join("integration_#{:rand.uniform(100000)}")
      competitor_folder = Path.join([tmp_dir, "Gym A", "Session 1", "1022 Kevin S"])
      File.mkdir_p!(competitor_folder)

      # Copy a real test JPEG (or create a minimal valid one)
      jpeg_path = Path.join(competitor_folder, "IMG_001.jpg")
      # Create minimal valid JPEG (1x1 pixel)
      File.write!(jpeg_path, minimal_jpeg())

      # Create event with competitor
      {:ok, event} = Ash.create(PhotoFinish.Events.Event, %{
        name: "Integration Test",
        slug: "integration-test",
        storage_directory: tmp_dir
      })

      {:ok, _competitor} = Ash.create(PhotoFinish.Events.Competitor, %{
        event_id: event.id,
        competitor_number: "1022",
        first_name: "Kevin",
        last_name: "S"
      })

      on_exit(fn -> File.rm_rf!(tmp_dir) end)

      %{event: event}
    end

    test "scan discovers photos and queues processing", %{event: event} do
      {:ok, result} = Ingestion.scan_event(event.id)

      assert result.photos_new == 1

      # Verify Oban job was queued
      assert_enqueued(worker: PhotoProcessor)
    end

    defp minimal_jpeg do
      # Minimal valid JPEG (1x1 white pixel)
      <<0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
        0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
        0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
        0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
        0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
        0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
        0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
        0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
        0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
        0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
        0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
        0x00, 0x00, 0x3F, 0x00, 0xFB, 0xD5, 0xDB, 0x20, 0xA8, 0xF1, 0x7C, 0xD7,
        0xFF, 0xD9>>
    end
  end
end
```

**Step 2: Run integration test**

Run: `cd server && mix test test/photo_finish/ingestion_integration_test.exs`
Expected: Test passes

**Step 3: Run full test suite**

Run: `cd server && mix test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add test/photo_finish/ingestion_integration_test.exs
git commit -m "test(ingestion): add integration test for full scan flow"
```

---

## Summary

After completing all tasks, the ingestion pipeline will:

1. **Manual Scan** - Admin clicks "Scan Now" on event page
2. **Directory Walking** - Scanner finds all JPEGs under event's storage_directory
3. **Hierarchy Creation** - Folder structure mirrored to hierarchy_nodes
4. **Photo Records** - Photo records created with discovered status
5. **Competitor Matching** - Leaf folders matched to roster (or left unlinked)
6. **Background Processing** - Oban jobs generate thumbnails and previews
7. **Status Updates** - Photos progress: discovered → processing → ready (or error)
