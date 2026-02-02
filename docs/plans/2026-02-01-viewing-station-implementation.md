# Viewing Station MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build search-first public viewing station with competitor import and folder-to-competitor association.

**Architecture:** Add session field to existing Competitor, build import UI for .txt rosters, build admin UI for associating photo folders to competitors, build public LiveView for search and photo browsing.

**Tech Stack:** Elixir/Phoenix, Ash Framework, LiveView, Tailwind CSS

---

## Task 1: Add Session Field to Competitor

**Files:**
- Modify: `server/lib/photo_finish/events/competitor.ex`
- Test: `server/test/photo_finish/events/competitor_test.exs`

**Step 1: Add session attribute to Competitor resource**

In `server/lib/photo_finish/events/competitor.ex`, add to the attributes block:

```elixir
attribute :session, :string do
  public? true
  description "Session identifier, e.g. '3A', '11B'"
end
```

Add `:session` to the create and update action accept lists.

**Step 2: Generate and run migration**

```bash
cd server && mix ash.codegen add_session_to_competitors && mix ecto.migrate
```

**Step 3: Write test for session field**

In `server/test/photo_finish/events/competitor_test.exs`:

```elixir
describe "session field" do
  test "can create competitor with session" do
    {:ok, event} = create_test_event()

    {:ok, competitor} =
      Ash.create(PhotoFinish.Events.Competitor, %{
        event_id: event.id,
        competitor_number: "1022",
        first_name: "Kevin",
        last_name: "S",
        session: "3A"
      })

    assert competitor.session == "3A"
  end
end
```

**Step 4: Run tests**

```bash
cd server && mix test test/photo_finish/events/competitor_test.exs
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add session field to Competitor"
```

---

## Task 2: Build Roster Parser Module

**Files:**
- Create: `server/lib/photo_finish/events/roster_parser.ex`
- Test: `server/test/photo_finish/events/roster_parser_test.exs`

**Step 1: Write failing test for parser**

Create `server/test/photo_finish/events/roster_parser_test.exs`:

```elixir
defmodule PhotoFinish.Events.RosterParserTest do
  use ExUnit.Case, async: true

  alias PhotoFinish.Events.RosterParser

  describe "parse_txt/1" do
    test "parses simple roster format" do
      content = """
      143 Avery W
      169 Callie W
      1022 Kevin S
      """

      result = RosterParser.parse_txt(content)

      assert {:ok, competitors} = result
      assert length(competitors) == 3

      assert Enum.at(competitors, 0) == %{
               competitor_number: "143",
               first_name: "Avery",
               last_name: "W"
             }

      assert Enum.at(competitors, 2) == %{
               competitor_number: "1022",
               first_name: "Kevin",
               last_name: "S"
             }
    end

    test "handles single-word names" do
      content = "143 Avery\n"

      {:ok, [competitor]} = RosterParser.parse_txt(content)

      assert competitor.competitor_number == "143"
      assert competitor.first_name == "Avery"
      assert competitor.last_name == nil
    end

    test "skips blank lines" do
      content = """
      143 Avery W

      169 Callie W
      """

      {:ok, competitors} = RosterParser.parse_txt(content)
      assert length(competitors) == 2
    end

    test "returns error for invalid lines" do
      content = "not a valid line\n"

      assert {:error, _} = RosterParser.parse_txt(content)
    end
  end
end
```

**Step 2: Run test to verify it fails**

```bash
cd server && mix test test/photo_finish/events/roster_parser_test.exs
```

Expected: FAIL (module not found)

**Step 3: Implement parser**

Create `server/lib/photo_finish/events/roster_parser.ex`:

```elixir
defmodule PhotoFinish.Events.RosterParser do
  @moduledoc """
  Parses roster files into competitor data.
  """

  @doc """
  Parse a .txt roster file with format: NUMBER NAME
  Returns {:ok, list} or {:error, reason}
  """
  def parse_txt(content) when is_binary(content) do
    lines =
      content
      |> String.split("\n")
      |> Enum.map(&String.trim/1)
      |> Enum.reject(&(&1 == ""))

    results = Enum.map(lines, &parse_line/1)

    errors = Enum.filter(results, &match?({:error, _}, &1))

    if Enum.empty?(errors) do
      competitors = Enum.map(results, fn {:ok, c} -> c end)
      {:ok, competitors}
    else
      {:error, "Invalid lines found"}
    end
  end

  defp parse_line(line) do
    case String.split(line, " ", parts: 2) do
      [number, name] when byte_size(number) > 0 and byte_size(name) > 0 ->
        {first_name, last_name} = split_name(name)

        {:ok,
         %{
           competitor_number: number,
           first_name: first_name,
           last_name: last_name
         }}

      _ ->
        {:error, "Invalid line: #{line}"}
    end
  end

  defp split_name(name) do
    case String.split(name, " ", parts: 2) do
      [first, last] -> {first, last}
      [first] -> {first, nil}
    end
  end
end
```

**Step 4: Run tests**

```bash
cd server && mix test test/photo_finish/events/roster_parser_test.exs
```

Expected: PASS

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add RosterParser for .txt roster files"
```

---

## Task 3: Build Roster Import Context Function

**Files:**
- Modify: `server/lib/photo_finish/events.ex`
- Create: `server/lib/photo_finish/events/roster_import.ex`
- Test: `server/test/photo_finish/events/roster_import_test.exs`

**Step 1: Write failing test**

Create `server/test/photo_finish/events/roster_import_test.exs`:

```elixir
defmodule PhotoFinish.Events.RosterImportTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.RosterImport
  alias PhotoFinish.Events.Competitor

  describe "import_roster/3" do
    test "creates competitors from roster content" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery W
      169 Callie W
      """

      {:ok, result} = RosterImport.import_roster(event.id, "3A", content)

      assert result.imported_count == 2
      assert result.error_count == 0

      # Verify competitors were created
      competitors = Ash.read!(Competitor) |> Enum.filter(&(&1.event_id == event.id))
      assert length(competitors) == 2

      avery = Enum.find(competitors, &(&1.competitor_number == "143"))
      assert avery.first_name == "Avery"
      assert avery.last_name == "W"
      assert avery.session == "3A"
      assert avery.display_name == "143 Avery W"
    end

    test "generates display_name from number and name" do
      {:ok, event} = create_test_event()

      content = "1022 Kevin S\n"

      {:ok, _} = RosterImport.import_roster(event.id, "11B", content)

      [competitor] = Ash.read!(Competitor) |> Enum.filter(&(&1.event_id == event.id))
      assert competitor.display_name == "1022 Kevin S"
    end
  end

  defp create_test_event do
    Ash.create(PhotoFinish.Events.Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end
end
```

**Step 2: Run test to verify it fails**

```bash
cd server && mix test test/photo_finish/events/roster_import_test.exs
```

**Step 3: Implement import module**

Create `server/lib/photo_finish/events/roster_import.ex`:

```elixir
defmodule PhotoFinish.Events.RosterImport do
  @moduledoc """
  Handles importing competitor rosters from file content.
  """

  alias PhotoFinish.Events.{Competitor, RosterParser}

  @doc """
  Import competitors from .txt roster content.

  Returns {:ok, %{imported_count: n, error_count: n}} or {:error, reason}
  """
  def import_roster(event_id, session, content) when is_binary(content) do
    with {:ok, parsed} <- RosterParser.parse_txt(content) do
      results =
        Enum.map(parsed, fn competitor_data ->
          create_competitor(event_id, session, competitor_data)
        end)

      imported = Enum.count(results, &match?({:ok, _}, &1))
      errors = Enum.count(results, &match?({:error, _}, &1))

      {:ok, %{imported_count: imported, error_count: errors}}
    end
  end

  defp create_competitor(event_id, session, %{
         competitor_number: number,
         first_name: first,
         last_name: last
       }) do
    display_name = build_display_name(number, first, last)

    Ash.create(Competitor, %{
      event_id: event_id,
      session: session,
      competitor_number: number,
      first_name: first,
      last_name: last,
      display_name: display_name
    })
  end

  defp build_display_name(number, first, nil), do: "#{number} #{first}"
  defp build_display_name(number, first, last), do: "#{number} #{first} #{last}"
end
```

**Step 4: Run tests**

```bash
cd server && mix test test/photo_finish/events/roster_import_test.exs
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add RosterImport for importing competitor rosters"
```

---

## Task 4: Build Roster Import LiveView UI

**Files:**
- Create: `server/lib/photo_finish_web/live/admin/competitor_live/import.ex`
- Modify: `server/lib/photo_finish_web/router.ex`

**Step 1: Add route**

In `server/lib/photo_finish_web/router.ex`, add inside the admin scope:

```elixir
live "/events/:event_id/competitors/import", CompetitorLive.Import, :import
```

**Step 2: Create Import LiveView**

Create `server/lib/photo_finish_web/live/admin/competitor_live/import.ex`:

```elixir
defmodule PhotoFinishWeb.Admin.CompetitorLive.Import do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events
  alias PhotoFinish.Events.Event
  alias PhotoFinish.Events.RosterImport

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(Event, event_id)

    socket =
      socket
      |> assign(:event, event)
      |> assign(:session, "")
      |> assign(:file_content, nil)
      |> assign(:filename, nil)
      |> assign(:preview, nil)
      |> assign(:result, nil)
      |> allow_upload(:roster, accept: ~w(.txt), max_entries: 1)

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="max-w-2xl mx-auto">
        <.link
          navigate={~p"/admin/events/#{@event.id}"}
          class="text-sm text-gray-500 hover:text-gray-700 flex items-center gap-1 mb-4"
        >
          <.icon name="hero-arrow-left" class="w-4 h-4" /> Back to Event
        </.link>

        <h1 class="text-2xl font-bold mb-6">Import Roster</h1>
        <p class="text-gray-600 mb-6">
          Import competitors for <strong><%= @event.name %></strong>
        </p>

        <form phx-submit="import" phx-change="validate" class="space-y-6">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Session
            </label>
            <input
              type="text"
              name="session"
              value={@session}
              placeholder="e.g. 3A, 11B"
              class="w-32 rounded-md border-gray-300 shadow-sm focus:border-indigo-500 focus:ring-indigo-500"
              required
            />
            <p class="mt-1 text-xs text-gray-500">
              The session these competitors belong to
            </p>
          </div>

          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Roster File (.txt)
            </label>
            <.live_file_input upload={@uploads.roster} class="block w-full text-sm text-gray-500
              file:mr-4 file:py-2 file:px-4
              file:rounded-md file:border-0
              file:text-sm file:font-semibold
              file:bg-indigo-50 file:text-indigo-700
              hover:file:bg-indigo-100" />
            <p class="mt-1 text-xs text-gray-500">
              Format: one competitor per line, "NUMBER NAME"
            </p>
          </div>

          <%= if @preview do %>
            <div class="border rounded-lg overflow-hidden">
              <div class="bg-gray-50 px-4 py-2 border-b">
                <span class="font-medium">Preview</span>
                <span class="text-gray-500 text-sm ml-2">
                  <%= length(@preview) %> competitors found
                </span>
              </div>
              <div class="max-h-64 overflow-y-auto">
                <table class="min-w-full divide-y divide-gray-200">
                  <thead class="bg-gray-50">
                    <tr>
                      <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">#</th>
                      <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">Number</th>
                      <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">Name</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-gray-200">
                    <%= for {competitor, idx} <- Enum.with_index(@preview) do %>
                      <tr>
                        <td class="px-4 py-2 text-sm text-gray-500"><%= idx + 1 %></td>
                        <td class="px-4 py-2 text-sm font-mono"><%= competitor.competitor_number %></td>
                        <td class="px-4 py-2 text-sm">
                          <%= competitor.first_name %> <%= competitor.last_name %>
                        </td>
                      </tr>
                    <% end %>
                  </tbody>
                </table>
              </div>
            </div>
          <% end %>

          <%= if @result do %>
            <div class={[
              "rounded-lg p-4",
              @result.error_count == 0 && "bg-green-50 text-green-800",
              @result.error_count > 0 && "bg-yellow-50 text-yellow-800"
            ]}>
              <p class="font-medium">
                Import complete: <%= @result.imported_count %> competitors imported
                <%= if @result.error_count > 0 do %>
                  (<%= @result.error_count %> errors)
                <% end %>
              </p>
            </div>
          <% end %>

          <div class="flex gap-4">
            <button
              type="submit"
              disabled={@preview == nil or @session == ""}
              class="px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Import Competitors
            </button>
          </div>
        </form>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def handle_event("validate", %{"session" => session}, socket) do
    socket = assign(socket, :session, session)

    socket =
      case uploaded_entries(socket, :roster) do
        {[entry], []} ->
          content = read_upload(socket, entry)
          parse_and_preview(socket, content)

        _ ->
          assign(socket, preview: nil, file_content: nil)
      end

    {:noreply, socket}
  end

  def handle_event("import", %{"session" => session}, socket) do
    socket = assign(socket, :session, session)

    {completed, []} = uploaded_entries(socket, :roster)

    socket =
      case completed do
        [entry] ->
          content = read_upload(socket, entry)

          case RosterImport.import_roster(socket.assigns.event.id, session, content) do
            {:ok, result} ->
              socket
              |> assign(:result, result)
              |> assign(:preview, nil)

            {:error, reason} ->
              put_flash(socket, :error, "Import failed: #{reason}")
          end

        _ ->
          put_flash(socket, :error, "Please select a file")
      end

    {:noreply, socket}
  end

  defp read_upload(socket, entry) do
    consume_uploaded_entry(socket, entry, fn %{path: path} ->
      {:ok, File.read!(path)}
    end)
  end

  defp parse_and_preview(socket, content) do
    case PhotoFinish.Events.RosterParser.parse_txt(content) do
      {:ok, competitors} ->
        socket
        |> assign(:preview, competitors)
        |> assign(:file_content, content)

      {:error, _} ->
        socket
        |> assign(:preview, nil)
        |> put_flash(:error, "Could not parse file")
    end
  end
end
```

**Step 3: Add link from Event show page**

In `server/lib/photo_finish_web/live/admin/event_live/show.ex`, add a link to import:

```elixir
<.link
  navigate={~p"/admin/events/#{@event.id}/competitors/import"}
  class="inline-flex items-center px-3 py-2 border border-gray-300 shadow-sm text-sm leading-4 font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50"
>
  <.icon name="hero-arrow-up-tray" class="w-4 h-4 mr-2" />
  Import Roster
</.link>
```

**Step 4: Test manually**

```bash
cd server && mix phx.server
```

Navigate to `/admin/events/{id}/competitors/import`

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add roster import UI"
```

---

## Task 5: Build Folder Association Context Functions

**Files:**
- Create: `server/lib/photo_finish/events/folder_association.ex`
- Test: `server/test/photo_finish/events/folder_association_test.exs`

**Step 1: Write failing tests**

Create `server/test/photo_finish/events/folder_association_test.exs`:

```elixir
defmodule PhotoFinish.Events.FolderAssociationTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.FolderAssociation
  alias PhotoFinish.Events.Competitor
  alias PhotoFinish.Photos.Photo

  describe "list_unassigned_folders/2" do
    test "returns folders with photos not yet assigned to competitors" do
      {:ok, event} = create_test_event()

      # Create photos in different source folders
      create_photo(event.id, "A", "Gym 01", "3A", "Group 3A", "Beam")
      create_photo(event.id, "A", "Gym 01", "3A", "Group 3A", "Beam")
      create_photo(event.id, "A", "Gym 02", "3A", "Group 3A", "Beam")

      folders = FolderAssociation.list_unassigned_folders(event.id, %{
        gym: "A",
        session: "3A",
        group_name: "Group 3A",
        apparatus: "Beam"
      })

      assert length(folders) == 2
      assert Enum.find(folders, &(&1.source_folder == "Gym 01")).photo_count == 2
      assert Enum.find(folders, &(&1.source_folder == "Gym 02")).photo_count == 1
    end
  end

  describe "list_session_competitors/2" do
    test "returns competitors for the given session" do
      {:ok, event} = create_test_event()
      {:ok, _c1} = create_competitor(event.id, "3A", "1022", "Kevin", "S")
      {:ok, _c2} = create_competitor(event.id, "3A", "1023", "Sarah", "J")
      {:ok, _c3} = create_competitor(event.id, "4A", "1024", "Emma", "W")

      competitors = FolderAssociation.list_session_competitors(event.id, "3A")

      assert length(competitors) == 2
      assert Enum.all?(competitors, &(&1.session == "3A"))
    end
  end

  describe "assign_folder/3" do
    test "sets competitor_id on all photos in folder" do
      {:ok, event} = create_test_event()
      {:ok, competitor} = create_competitor(event.id, "3A", "1022", "Kevin", "S")
      {:ok, photo1} = create_photo(event.id, "A", "Gym 01", "3A", "Group 3A", "Beam")
      {:ok, photo2} = create_photo(event.id, "A", "Gym 01", "3A", "Group 3A", "Beam")
      {:ok, photo3} = create_photo(event.id, "A", "Gym 02", "3A", "Group 3A", "Beam")

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", competitor.id)

      assert count == 2

      # Reload photos and check
      updated1 = Ash.get!(Photo, photo1.id)
      updated2 = Ash.get!(Photo, photo2.id)
      updated3 = Ash.get!(Photo, photo3.id)

      assert updated1.competitor_id == competitor.id
      assert updated2.competitor_id == competitor.id
      assert updated3.competitor_id == nil
    end
  end

  defp create_test_event do
    Ash.create(PhotoFinish.Events.Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end

  defp create_competitor(event_id, session, number, first, last) do
    Ash.create(Competitor, %{
      event_id: event_id,
      session: session,
      competitor_number: number,
      first_name: first,
      last_name: last,
      display_name: "#{number} #{first} #{last}"
    })
  end

  defp create_photo(event_id, gym, source_folder, session, group_name, apparatus) do
    Ash.create(Photo, %{
      event_id: event_id,
      gym: gym,
      session: session,
      group_name: group_name,
      apparatus: apparatus,
      source_folder: source_folder,
      ingestion_path: "/tmp/#{System.unique_integer([:positive])}.jpg",
      filename: "photo_#{System.unique_integer([:positive])}.jpg"
    })
  end
end
```

**Step 2: Run test to verify it fails**

```bash
cd server && mix test test/photo_finish/events/folder_association_test.exs
```

**Step 3: Implement FolderAssociation**

Create `server/lib/photo_finish/events/folder_association.ex`:

```elixir
defmodule PhotoFinish.Events.FolderAssociation do
  @moduledoc """
  Handles associating photo folders to competitors.
  """

  import Ecto.Query
  alias PhotoFinish.Repo
  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Events.Competitor

  @doc """
  List folders (source_folder values) at a given location that have
  photos not yet assigned to a competitor.
  """
  def list_unassigned_folders(event_id, %{
        gym: gym,
        session: session,
        group_name: group_name,
        apparatus: apparatus
      }) do
    query =
      from p in Photo,
        where: p.event_id == ^event_id,
        where: p.gym == ^gym,
        where: p.session == ^session,
        where: p.group_name == ^group_name,
        where: p.apparatus == ^apparatus,
        where: is_nil(p.competitor_id),
        where: not is_nil(p.source_folder),
        group_by: p.source_folder,
        select: %{
          source_folder: p.source_folder,
          photo_count: count(p.id)
        },
        order_by: p.source_folder

    Repo.all(query)
  end

  @doc """
  List competitors for a given event and session.
  """
  def list_session_competitors(event_id, session) do
    Competitor
    |> Ash.Query.filter(event_id == ^event_id and session == ^session)
    |> Ash.Query.sort(:competitor_number)
    |> Ash.read!()
  end

  @doc """
  Assign all photos in a source_folder to a competitor.
  Returns {:ok, count} with number of photos updated.
  """
  def assign_folder(event_id, source_folder, competitor_id) do
    {count, _} =
      from(p in Photo,
        where: p.event_id == ^event_id,
        where: p.source_folder == ^source_folder,
        where: is_nil(p.competitor_id)
      )
      |> Repo.update_all(set: [competitor_id: competitor_id, updated_at: DateTime.utc_now()])

    {:ok, count}
  end
end
```

**Step 4: Run tests**

```bash
cd server && mix test test/photo_finish/events/folder_association_test.exs
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add FolderAssociation for linking photos to competitors"
```

---

## Task 6: Build Folder Association LiveView UI

**Files:**
- Create: `server/lib/photo_finish_web/live/admin/folder_live/associate.ex`
- Modify: `server/lib/photo_finish_web/router.ex`

**Step 1: Add route**

In router.ex, add inside admin scope:

```elixir
live "/events/:event_id/associate", FolderLive.Associate, :associate
```

**Step 2: Create Associate LiveView**

Create `server/lib/photo_finish_web/live/admin/folder_live/associate.ex`:

```elixir
defmodule PhotoFinishWeb.Admin.FolderLive.Associate do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Events.FolderAssociation
  alias PhotoFinish.Photos.LocationBrowser

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(Event, event_id)

    socket =
      socket
      |> assign(:event, event)
      |> assign(:path, [])
      |> assign(:children, LocationBrowser.get_children(event_id, []))
      |> assign(:folders, [])
      |> assign(:competitors, [])
      |> assign(:selected_folder, nil)
      |> assign(:assignments, %{})

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="max-w-6xl mx-auto">
        <.link
          navigate={~p"/admin/events/#{@event.id}"}
          class="text-sm text-gray-500 hover:text-gray-700 flex items-center gap-1 mb-4"
        >
          <.icon name="hero-arrow-left" class="w-4 h-4" /> Back to Event
        </.link>

        <h1 class="text-2xl font-bold mb-2">Associate Folders to Competitors</h1>
        <p class="text-gray-600 mb-6"><%= @event.name %></p>

        <!-- Breadcrumb Navigation -->
        <div class="flex items-center gap-2 mb-6 text-sm">
          <button phx-click="nav" phx-value-level="-1" class="text-indigo-600 hover:underline">
            Event
          </button>
          <%= for {item, idx} <- Enum.with_index(@path) do %>
            <span class="text-gray-400">/</span>
            <button
              phx-click="nav"
              phx-value-level={idx}
              class="text-indigo-600 hover:underline"
            >
              <%= item %>
            </button>
          <% end %>
        </div>

        <%= if length(@path) < 4 do %>
          <!-- Hierarchy Navigation -->
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <%= for child <- @children do %>
              <button
                phx-click="drill"
                phx-value-name={child.name}
                class="p-4 bg-white rounded-lg shadow hover:shadow-md text-left"
              >
                <div class="font-medium"><%= child.name %></div>
                <div class="text-sm text-gray-500"><%= child.count %> photos</div>
              </button>
            <% end %>
          </div>
        <% else %>
          <!-- Folder Association UI -->
          <div class="grid grid-cols-2 gap-8">
            <!-- Left: Unassigned Folders -->
            <div>
              <h2 class="font-semibold text-lg mb-4">Unassigned Folders</h2>
              <div class="space-y-2">
                <%= for folder <- @folders do %>
                  <button
                    phx-click="select_folder"
                    phx-value-folder={folder.source_folder}
                    class={[
                      "w-full p-3 rounded-lg text-left flex justify-between items-center",
                      @selected_folder == folder.source_folder && "bg-indigo-100 border-2 border-indigo-500",
                      @selected_folder != folder.source_folder && "bg-white border border-gray-200 hover:border-gray-300",
                      Map.has_key?(@assignments, folder.source_folder) && "opacity-50"
                    ]}
                    disabled={Map.has_key?(@assignments, folder.source_folder)}
                  >
                    <span class="font-mono"><%= folder.source_folder %></span>
                    <span class="text-sm text-gray-500"><%= folder.photo_count %> photos</span>
                  </button>
                <% end %>

                <%= if @folders == [] do %>
                  <p class="text-gray-500 italic">No unassigned folders</p>
                <% end %>
              </div>
            </div>

            <!-- Right: Competitors -->
            <div>
              <h2 class="font-semibold text-lg mb-4">
                Session <%= Enum.at(@path, 1) %> Competitors
              </h2>
              <div class="space-y-1 max-h-96 overflow-y-auto">
                <%= for competitor <- @competitors do %>
                  <button
                    phx-click="assign"
                    phx-value-competitor-id={competitor.id}
                    disabled={@selected_folder == nil}
                    class={[
                      "w-full p-2 rounded text-left flex items-center gap-3",
                      @selected_folder && "hover:bg-indigo-50 cursor-pointer",
                      !@selected_folder && "opacity-50 cursor-not-allowed"
                    ]}
                  >
                    <span class="font-mono text-sm w-16"><%= competitor.competitor_number %></span>
                    <span><%= competitor.first_name %> <%= competitor.last_name %></span>
                  </button>
                <% end %>

                <%= if @competitors == [] do %>
                  <p class="text-gray-500 italic">No competitors for this session</p>
                <% end %>
              </div>
            </div>
          </div>

          <!-- Pending Assignments -->
          <%= if map_size(@assignments) > 0 do %>
            <div class="mt-8 p-4 bg-gray-50 rounded-lg">
              <h3 class="font-medium mb-3">Pending Assignments (<%= map_size(@assignments) %>)</h3>
              <div class="space-y-1 text-sm">
                <%= for {folder, competitor} <- @assignments do %>
                  <div class="flex items-center gap-2">
                    <span class="font-mono"><%= folder %></span>
                    <span class="text-gray-400">→</span>
                    <span><%= competitor.display_name %></span>
                    <button
                      phx-click="unassign"
                      phx-value-folder={folder}
                      class="text-red-600 hover:underline ml-2"
                    >
                      Remove
                    </button>
                  </div>
                <% end %>
              </div>
              <button
                phx-click="save_assignments"
                class="mt-4 px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700"
              >
                Save All Assignments
              </button>
            </div>
          <% end %>
        <% end %>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def handle_event("drill", %{"name" => name}, socket) do
    new_path = socket.assigns.path ++ [name]

    socket =
      socket
      |> assign(:path, new_path)
      |> assign(:selected_folder, nil)
      |> load_data_for_path(new_path)

    {:noreply, socket}
  end

  def handle_event("nav", %{"level" => level}, socket) do
    level = String.to_integer(level)
    new_path = if level < 0, do: [], else: Enum.take(socket.assigns.path, level + 1)

    socket =
      socket
      |> assign(:path, new_path)
      |> assign(:selected_folder, nil)
      |> assign(:assignments, %{})
      |> load_data_for_path(new_path)

    {:noreply, socket}
  end

  def handle_event("select_folder", %{"folder" => folder}, socket) do
    {:noreply, assign(socket, :selected_folder, folder)}
  end

  def handle_event("assign", %{"competitor-id" => competitor_id}, socket) do
    folder = socket.assigns.selected_folder
    competitor = Enum.find(socket.assigns.competitors, &(&1.id == competitor_id))

    assignments = Map.put(socket.assigns.assignments, folder, competitor)

    socket =
      socket
      |> assign(:assignments, assignments)
      |> assign(:selected_folder, nil)

    {:noreply, socket}
  end

  def handle_event("unassign", %{"folder" => folder}, socket) do
    assignments = Map.delete(socket.assigns.assignments, folder)
    {:noreply, assign(socket, :assignments, assignments)}
  end

  def handle_event("save_assignments", _params, socket) do
    event_id = socket.assigns.event.id

    results =
      for {folder, competitor} <- socket.assigns.assignments do
        FolderAssociation.assign_folder(event_id, folder, competitor.id)
      end

    total = Enum.sum(for {:ok, count} <- results, do: count)

    socket =
      socket
      |> assign(:assignments, %{})
      |> put_flash(:info, "Assigned #{total} photos")
      |> load_data_for_path(socket.assigns.path)

    {:noreply, socket}
  end

  defp load_data_for_path(socket, path) when length(path) < 4 do
    children = LocationBrowser.get_children(socket.assigns.event.id, path)

    socket
    |> assign(:children, children)
    |> assign(:folders, [])
    |> assign(:competitors, [])
  end

  defp load_data_for_path(socket, [gym, session, group_name, apparatus]) do
    event_id = socket.assigns.event.id

    folders =
      FolderAssociation.list_unassigned_folders(event_id, %{
        gym: gym,
        session: session,
        group_name: group_name,
        apparatus: apparatus
      })

    competitors = FolderAssociation.list_session_competitors(event_id, session)

    socket
    |> assign(:children, [])
    |> assign(:folders, folders)
    |> assign(:competitors, competitors)
  end
end
```

**Step 3: Test manually**

```bash
cd server && mix phx.server
```

Navigate to `/admin/events/{id}/associate`

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add folder-to-competitor association UI"
```

---

## Task 7: Build Viewer Search Context Function

**Files:**
- Create: `server/lib/photo_finish/viewer.ex`
- Create: `server/lib/photo_finish/viewer/search.ex`
- Test: `server/test/photo_finish/viewer/search_test.exs`

**Step 1: Write failing tests**

Create `server/test/photo_finish/viewer/search_test.exs`:

```elixir
defmodule PhotoFinish.Viewer.SearchTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Viewer.Search

  describe "search_competitors/2" do
    test "finds competitors by number" do
      {:ok, event} = create_test_event()
      {:ok, _c1} = create_competitor_with_photos(event.id, "1022", "Kevin", "S", 5)
      {:ok, _c2} = create_competitor_with_photos(event.id, "1023", "Sarah", "J", 3)

      results = Search.search_competitors(event.id, "1022")

      assert length(results) == 1
      assert hd(results).competitor_number == "1022"
      assert hd(results).photo_count == 5
    end

    test "finds competitors by first name" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_competitor_with_photos(event.id, "1022", "Kevin", "Sherman", 5)

      results = Search.search_competitors(event.id, "kevin")

      assert length(results) == 1
      assert hd(results).first_name == "Kevin"
    end

    test "finds competitors by last name" do
      {:ok, event} = create_test_event()
      {:ok, _} = create_competitor_with_photos(event.id, "1022", "Kevin", "Sherman", 5)

      results = Search.search_competitors(event.id, "sherman")

      assert length(results) == 1
    end

    test "returns empty list when no matches" do
      {:ok, event} = create_test_event()

      results = Search.search_competitors(event.id, "nobody")

      assert results == []
    end

    test "limits results to 10" do
      {:ok, event} = create_test_event()

      for i <- 1..15 do
        create_competitor_with_photos(event.id, "10#{i}", "Test", "#{i}", 1)
      end

      results = Search.search_competitors(event.id, "Test")

      assert length(results) == 10
    end
  end

  defp create_test_event do
    Ash.create(PhotoFinish.Events.Event, %{
      name: "Test Event",
      slug: "test-event-#{System.unique_integer([:positive])}",
      storage_root: "/tmp/test"
    })
  end

  defp create_competitor_with_photos(event_id, number, first, last, photo_count) do
    {:ok, competitor} =
      Ash.create(PhotoFinish.Events.Competitor, %{
        event_id: event_id,
        competitor_number: number,
        first_name: first,
        last_name: last,
        session: "1A",
        display_name: "#{number} #{first} #{last}"
      })

    for _ <- 1..photo_count do
      Ash.create!(PhotoFinish.Photos.Photo, %{
        event_id: event_id,
        competitor_id: competitor.id,
        ingestion_path: "/tmp/#{System.unique_integer([:positive])}.jpg",
        filename: "photo.jpg",
        status: :ready
      })
    end

    {:ok, competitor}
  end
end
```

**Step 2: Run test to verify it fails**

```bash
cd server && mix test test/photo_finish/viewer/search_test.exs
```

**Step 3: Create Viewer domain**

Create `server/lib/photo_finish/viewer.ex`:

```elixir
defmodule PhotoFinish.Viewer do
  @moduledoc """
  Public viewer functionality.
  """
end
```

Create `server/lib/photo_finish/viewer/search.ex`:

```elixir
defmodule PhotoFinish.Viewer.Search do
  @moduledoc """
  Search functionality for the public viewer.
  """

  import Ecto.Query
  alias PhotoFinish.Repo
  alias PhotoFinish.Events.Competitor
  alias PhotoFinish.Photos.Photo

  @max_results 10

  @doc """
  Search competitors by number, first name, or last name.
  Returns competitors with photo counts.
  """
  def search_competitors(event_id, query) when is_binary(query) do
    query = String.trim(query)

    if String.length(query) < 1 do
      []
    else
      pattern = "%#{query}%"

      from(c in Competitor,
        where: c.event_id == ^event_id,
        where: c.is_active == true,
        where:
          ilike(c.competitor_number, ^pattern) or
            ilike(c.first_name, ^pattern) or
            ilike(c.last_name, ^pattern),
        left_join: p in Photo,
        on: p.competitor_id == c.id and p.status == :ready,
        group_by: c.id,
        select: %{
          id: c.id,
          competitor_number: c.competitor_number,
          first_name: c.first_name,
          last_name: c.last_name,
          display_name: c.display_name,
          session: c.session,
          photo_count: count(p.id)
        },
        order_by: [c.competitor_number],
        limit: @max_results
      )
      |> Repo.all()
    end
  end
end
```

**Step 4: Run tests**

```bash
cd server && mix test test/photo_finish/viewer/search_test.exs
```

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add Viewer.Search for competitor search"
```

---

## Task 8: Build Viewer Home LiveView (Search)

**Files:**
- Create: `server/lib/photo_finish_web/live/viewer_live/home.ex`
- Modify: `server/lib/photo_finish_web/router.ex`

**Step 1: Add routes**

In router.ex, add new scope for public viewer (no auth):

```elixir
scope "/view", PhotoFinishWeb do
  pipe_through :browser

  live "/", ViewerLive.Home, :home
  live "/competitor/:id", ViewerLive.Competitor, :show
end
```

**Step 2: Create Home LiveView**

Create `server/lib/photo_finish_web/live/viewer_live/home.ex`:

```elixir
defmodule PhotoFinishWeb.ViewerLive.Home do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Viewer.Search

  @impl true
  def mount(_params, _session, socket) do
    # For now, get the first active event
    # TODO: Support multiple events or event selection
    event = get_active_event()

    socket =
      socket
      |> assign(:event, event)
      |> assign(:query, "")
      |> assign(:results, [])
      |> assign(:searching, false)

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-screen bg-gray-100">
      <!-- Header -->
      <header class="bg-white shadow-sm">
        <div class="max-w-4xl mx-auto px-4 py-4 flex items-center justify-between">
          <h1 class="text-xl font-bold text-gray-900">PhotoFinish</h1>
        </div>
      </header>

      <!-- Main Content -->
      <main class="max-w-4xl mx-auto px-4 py-12">
        <div class="text-center mb-12">
          <%= if @event do %>
            <h2 class="text-3xl font-bold text-gray-900 mb-2"><%= @event.name %></h2>
          <% end %>
        </div>

        <!-- Search Box -->
        <div class="max-w-xl mx-auto">
          <form phx-change="search" phx-submit="search">
            <div class="relative">
              <input
                type="text"
                name="query"
                value={@query}
                placeholder="Search by name or number..."
                phx-debounce="300"
                autocomplete="off"
                class="w-full px-6 py-4 text-lg rounded-full border-2 border-gray-200 focus:border-indigo-500 focus:ring-0 shadow-sm"
              />
              <div class="absolute right-4 top-1/2 -translate-y-1/2">
                <%= if @searching do %>
                  <.icon name="hero-arrow-path" class="w-6 h-6 text-gray-400 animate-spin" />
                <% else %>
                  <.icon name="hero-magnifying-glass" class="w-6 h-6 text-gray-400" />
                <% end %>
              </div>
            </div>
          </form>

          <!-- Results -->
          <%= if @query != "" do %>
            <div class="mt-4 bg-white rounded-xl shadow-lg overflow-hidden">
              <%= if @results == [] do %>
                <div class="p-6 text-center text-gray-500">
                  No competitors found for "<%= @query %>"
                </div>
              <% else %>
                <ul class="divide-y divide-gray-100">
                  <%= for result <- @results do %>
                    <li>
                      <.link
                        navigate={~p"/view/competitor/#{result.id}"}
                        class="block px-6 py-4 hover:bg-gray-50 flex items-center justify-between"
                      >
                        <div>
                          <div class="font-medium text-gray-900">
                            <span class="font-mono mr-2"><%= result.competitor_number %></span>
                            <%= result.first_name %> <%= result.last_name %>
                          </div>
                          <div class="text-sm text-gray-500">
                            Session <%= result.session %>
                          </div>
                        </div>
                        <div class="flex items-center gap-2 text-gray-500">
                          <span><%= result.photo_count %> photos</span>
                          <.icon name="hero-chevron-right" class="w-5 h-5" />
                        </div>
                      </.link>
                    </li>
                  <% end %>
                </ul>
              <% end %>
            </div>
          <% end %>
        </div>
      </main>
    </div>
    """
  end

  @impl true
  def handle_event("search", %{"query" => query}, socket) do
    socket = assign(socket, :query, query)

    if socket.assigns.event && String.length(String.trim(query)) > 0 do
      results = Search.search_competitors(socket.assigns.event.id, query)
      {:noreply, assign(socket, :results, results)}
    else
      {:noreply, assign(socket, :results, [])}
    end
  end

  defp get_active_event do
    PhotoFinish.Events.Event
    |> Ash.Query.filter(status == :active)
    |> Ash.Query.limit(1)
    |> Ash.read!()
    |> List.first()
  end
end
```

**Step 3: Test manually**

```bash
cd server && mix phx.server
```

Navigate to `/view`

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add viewer home page with competitor search"
```

---

## Task 9: Build Viewer Competitor Photo Grid

**Files:**
- Create: `server/lib/photo_finish_web/live/viewer_live/competitor.ex`

**Step 1: Create Competitor LiveView**

Create `server/lib/photo_finish_web/live/viewer_live/competitor.ex`:

```elixir
defmodule PhotoFinishWeb.ViewerLive.Competitor do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Competitor
  alias PhotoFinish.Photos.Photo

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    competitor = Ash.get!(Competitor, id)
    photos = load_photos(competitor.id)

    socket =
      socket
      |> assign(:competitor, competitor)
      |> assign(:photos, photos)
      |> assign(:lightbox_photo, nil)
      |> assign(:lightbox_index, nil)

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-screen bg-gray-100">
      <!-- Header -->
      <header class="bg-white shadow-sm sticky top-0 z-10">
        <div class="max-w-6xl mx-auto px-4 py-4 flex items-center justify-between">
          <div class="flex items-center gap-4">
            <.link navigate={~p"/view"} class="text-gray-500 hover:text-gray-700">
              <.icon name="hero-arrow-left" class="w-6 h-6" />
            </.link>
            <div>
              <h1 class="text-lg font-bold text-gray-900">
                <%= @competitor.display_name %>
              </h1>
              <p class="text-sm text-gray-500"><%= length(@photos) %> photos</p>
            </div>
          </div>
        </div>
      </header>

      <!-- Photo Grid -->
      <main class="max-w-6xl mx-auto px-4 py-6">
        <div class="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-2">
          <%= for {photo, idx} <- Enum.with_index(@photos) do %>
            <button
              phx-click="open_lightbox"
              phx-value-index={idx}
              class="aspect-[3/2] bg-gray-200 rounded overflow-hidden hover:opacity-90 transition"
            >
              <img
                src={~p"/admin/photos/thumbnail/#{photo.id}"}
                loading="lazy"
                class="w-full h-full object-cover"
              />
            </button>
          <% end %>
        </div>

        <%= if @photos == [] do %>
          <div class="text-center py-12 text-gray-500">
            No photos available yet
          </div>
        <% end %>
      </main>

      <!-- Lightbox -->
      <%= if @lightbox_photo do %>
        <div
          class="fixed inset-0 bg-black/90 z-50 flex items-center justify-center"
          phx-window-keydown="lightbox_key"
        >
          <!-- Close button -->
          <button
            phx-click="close_lightbox"
            class="absolute top-4 right-4 text-white/70 hover:text-white"
          >
            <.icon name="hero-x-mark" class="w-8 h-8" />
          </button>

          <!-- Previous -->
          <%= if @lightbox_index > 0 do %>
            <button
              phx-click="lightbox_prev"
              class="absolute left-4 top-1/2 -translate-y-1/2 text-white/70 hover:text-white p-2"
            >
              <.icon name="hero-chevron-left" class="w-10 h-10" />
            </button>
          <% end %>

          <!-- Image -->
          <div class="max-w-4xl max-h-[80vh] px-16">
            <img
              src={~p"/admin/photos/preview/#{@lightbox_photo.id}"}
              class="max-w-full max-h-[80vh] object-contain"
            />
          </div>

          <!-- Next -->
          <%= if @lightbox_index < length(@photos) - 1 do %>
            <button
              phx-click="lightbox_next"
              class="absolute right-4 top-1/2 -translate-y-1/2 text-white/70 hover:text-white p-2"
            >
              <.icon name="hero-chevron-right" class="w-10 h-10" />
            </button>
          <% end %>

          <!-- Counter -->
          <div class="absolute bottom-4 left-1/2 -translate-x-1/2 text-white/70">
            <%= @lightbox_index + 1 %> of <%= length(@photos) %>
          </div>
        </div>
      <% end %>
    </div>
    """
  end

  @impl true
  def handle_event("open_lightbox", %{"index" => index}, socket) do
    index = String.to_integer(index)
    photo = Enum.at(socket.assigns.photos, index)

    socket =
      socket
      |> assign(:lightbox_photo, photo)
      |> assign(:lightbox_index, index)

    {:noreply, socket}
  end

  def handle_event("close_lightbox", _params, socket) do
    socket =
      socket
      |> assign(:lightbox_photo, nil)
      |> assign(:lightbox_index, nil)

    {:noreply, socket}
  end

  def handle_event("lightbox_prev", _params, socket) do
    new_index = max(0, socket.assigns.lightbox_index - 1)
    photo = Enum.at(socket.assigns.photos, new_index)

    socket =
      socket
      |> assign(:lightbox_photo, photo)
      |> assign(:lightbox_index, new_index)

    {:noreply, socket}
  end

  def handle_event("lightbox_next", _params, socket) do
    max_index = length(socket.assigns.photos) - 1
    new_index = min(max_index, socket.assigns.lightbox_index + 1)
    photo = Enum.at(socket.assigns.photos, new_index)

    socket =
      socket
      |> assign(:lightbox_photo, photo)
      |> assign(:lightbox_index, new_index)

    {:noreply, socket}
  end

  def handle_event("lightbox_key", %{"key" => "Escape"}, socket) do
    handle_event("close_lightbox", %{}, socket)
  end

  def handle_event("lightbox_key", %{"key" => "ArrowLeft"}, socket) do
    if socket.assigns.lightbox_index > 0 do
      handle_event("lightbox_prev", %{}, socket)
    else
      {:noreply, socket}
    end
  end

  def handle_event("lightbox_key", %{"key" => "ArrowRight"}, socket) do
    if socket.assigns.lightbox_index < length(socket.assigns.photos) - 1 do
      handle_event("lightbox_next", %{}, socket)
    else
      {:noreply, socket}
    end
  end

  def handle_event("lightbox_key", _params, socket) do
    {:noreply, socket}
  end

  defp load_photos(competitor_id) do
    Photo
    |> Ash.Query.filter(competitor_id == ^competitor_id and status == :ready)
    |> Ash.Query.sort(:filename)
    |> Ash.read!()
  end
end
```

**Step 2: Test manually**

```bash
cd server && mix phx.server
```

Search for a competitor, click through to see photo grid and lightbox.

**Step 3: Commit**

```bash
git add -A && git commit -m "feat: add viewer competitor page with photo grid and lightbox"
```

---

## Task 10: Add Public Photo Serving Routes

**Files:**
- Modify: `server/lib/photo_finish_web/router.ex`
- Create: `server/lib/photo_finish_web/controllers/viewer_photo_controller.ex`

Currently photos are served from `/admin/photos/...` which requires basic auth. We need public routes.

**Step 1: Create public photo controller**

Create `server/lib/photo_finish_web/controllers/viewer_photo_controller.ex`:

```elixir
defmodule PhotoFinishWeb.ViewerPhotoController do
  use PhotoFinishWeb, :controller

  alias PhotoFinish.Photos.Photo

  def thumbnail(conn, %{"id" => id}) do
    photo = Ash.get!(Photo, id)
    serve_file(conn, photo.thumbnail_path)
  end

  def preview(conn, %{"id" => id}) do
    photo = Ash.get!(Photo, id)
    serve_file(conn, photo.preview_path)
  end

  defp serve_file(conn, nil) do
    conn
    |> put_status(404)
    |> text("Not found")
  end

  defp serve_file(conn, path) do
    if File.exists?(path) do
      conn
      |> put_resp_content_type("image/jpeg")
      |> put_resp_header("cache-control", "public, max-age=31536000")
      |> send_file(200, path)
    else
      conn
      |> put_status(404)
      |> text("Not found")
    end
  end
end
```

**Step 2: Add routes**

In router.ex, add to the `/view` scope:

```elixir
get "/photos/thumbnail/:id", ViewerPhotoController, :thumbnail
get "/photos/preview/:id", ViewerPhotoController, :preview
```

**Step 3: Update Competitor LiveView to use public routes**

In `viewer_live/competitor.ex`, change:

```elixir
# From:
src={~p"/admin/photos/thumbnail/#{photo.id}"}
src={~p"/admin/photos/preview/#{@lightbox_photo.id}"}

# To:
src={~p"/view/photos/thumbnail/#{photo.id}"}
src={~p"/view/photos/preview/#{@lightbox_photo.id}"}
```

**Step 4: Test manually**

```bash
cd server && mix phx.server
```

Verify photos load without auth at `/view/competitor/{id}`

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add public photo serving routes for viewer"
```

---

## Summary

After completing all tasks, you will have:

1. **Competitor with session field** - Track which session competitors belong to
2. **Roster parser** - Parse .txt roster files
3. **Roster import** - Import competitors from roster files via admin UI
4. **Folder association** - Link photo folders to competitors via admin UI
5. **Viewer search** - Public search by name/number
6. **Viewer photo grid** - Display competitor photos with lightbox
7. **Public photo routes** - Serve photos without authentication

The viewing station will be accessible at `/view` with no login required.
