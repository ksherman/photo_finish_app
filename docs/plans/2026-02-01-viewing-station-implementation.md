# Viewing Station MVP Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build search-first public viewing station with competitor import and folder-to-competitor association.

**Architecture:** Create `event_competitors` join table, simplify `competitors` to person-only, update `photos` to reference `event_competitors`. Build import UI for .txt rosters, admin UI for folder association, and public LiveView for search/browsing.

**Tech Stack:** Elixir/Phoenix, Ash Framework, LiveView, Tailwind CSS

---

## Task 1: Refactor Data Model (Competitor → EventCompetitor split)

This is a breaking schema change. Since we're in development, we'll modify the resources and regenerate migrations.

**Files:**
- Create: `server/lib/photo_finish/events/event_competitor.ex`
- Modify: `server/lib/photo_finish/events/competitor.ex`
- Modify: `server/lib/photo_finish/photos/photo.ex`
- Modify: `server/lib/photo_finish/events.ex`
- Modify: `server/lib/photo_finish/id.ex`

**Step 1: Add ID generator for event_competitor**

In `server/lib/photo_finish/id.ex`, add:

```elixir
def event_competitor_id, do: generate("evc_")
```

**Step 2: Create EventCompetitor resource**

Create `server/lib/photo_finish/events/event_competitor.ex`:

```elixir
defmodule PhotoFinish.Events.EventCompetitor do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "event_competitors"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :competitor_id,
        :event_id,
        :session,
        :competitor_number,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :is_active,
        :metadata
      ],
      update: [
        :session,
        :competitor_number,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :is_active,
        :metadata
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.event_competitor_id/0
      writable? false
    end

    attribute :session, :string do
      public? true
      description "Session identifier, e.g. '3A', '11B'"
    end

    attribute :competitor_number, :string do
      allow_nil? false
      public? true
    end

    attribute :display_name, :string do
      public? true
    end

    attribute :team_name, :string do
      public? true
    end

    attribute :level, :string do
      public? true
    end

    attribute :age_group, :string do
      public? true
    end

    attribute :is_active, :boolean do
      public? true
      default true
    end

    attribute :metadata, :map do
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :competitor, PhotoFinish.Events.Competitor do
      public? true
      attribute_type :string
    end

    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      public? true
    end
  end

  identities do
    identity :unique_event_competitor_number, [:event_id, :competitor_number]
  end
end
```

**Step 3: Simplify Competitor resource**

Update `server/lib/photo_finish/events/competitor.ex` to remove event-specific fields:

```elixir
defmodule PhotoFinish.Events.Competitor do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "competitors"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [:first_name, :last_name, :external_id, :email, :phone, :metadata],
      update: [:first_name, :last_name, :external_id, :email, :phone, :metadata]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.competitor_id/0
      writable? false
    end

    attribute :first_name, :string do
      allow_nil? false
      public? true
    end

    attribute :last_name, :string do
      public? true
    end

    attribute :external_id, :string do
      public? true
      description "External ID like USAG number for cross-event linking"
    end

    attribute :email, :string do
      public? true
    end

    attribute :phone, :string do
      public? true
    end

    attribute :metadata, :map do
      public? true
    end

    timestamps()
  end

  relationships do
    has_many :event_competitors, PhotoFinish.Events.EventCompetitor do
      public? true
    end
  end
end
```

**Step 4: Update Photo resource**

In `server/lib/photo_finish/photos/photo.ex`, change the relationship:

Replace:
```elixir
belongs_to :competitor, PhotoFinish.Events.Competitor do
  public? true
  attribute_type :string
end
```

With:
```elixir
belongs_to :event_competitor, PhotoFinish.Events.EventCompetitor do
  public? true
  attribute_type :string
end
```

Also update the action accept lists to use `:event_competitor_id` instead of `:competitor_id`.

**Step 5: Register EventCompetitor in domain**

In `server/lib/photo_finish/events.ex`:

```elixir
defmodule PhotoFinish.Events do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Events.Event
    resource PhotoFinish.Events.Competitor
    resource PhotoFinish.Events.EventCompetitor
  end
end
```

**Step 6: Reset database and regenerate migrations**

Since we're in development with no production data:

```bash
cd server
mix ecto.drop
mix ash.codegen refactor_event_competitors
mix ecto.create
mix ecto.migrate
```

**Step 7: Write tests**

Create `server/test/photo_finish/events/event_competitor_test.exs`:

```elixir
defmodule PhotoFinish.Events.EventCompetitorTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}

  describe "create" do
    test "creates event_competitor linking person to event" do
      {:ok, event} = Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/tmp/test"
      })

      {:ok, competitor} = Ash.create(Competitor, %{
        first_name: "Kevin",
        last_name: "S"
      })

      {:ok, event_competitor} = Ash.create(EventCompetitor, %{
        competitor_id: competitor.id,
        event_id: event.id,
        competitor_number: "1022",
        session: "3A",
        display_name: "1022 Kevin S"
      })

      assert event_competitor.session == "3A"
      assert event_competitor.competitor_number == "1022"
      assert String.starts_with?(event_competitor.id, "evc_")
    end

    test "enforces unique competitor_number per event" do
      {:ok, event} = Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/tmp/test"
      })

      {:ok, c1} = Ash.create(Competitor, %{first_name: "Kevin"})
      {:ok, c2} = Ash.create(Competitor, %{first_name: "Sarah"})

      {:ok, _} = Ash.create(EventCompetitor, %{
        competitor_id: c1.id,
        event_id: event.id,
        competitor_number: "1022"
      })

      assert {:error, _} = Ash.create(EventCompetitor, %{
        competitor_id: c2.id,
        event_id: event.id,
        competitor_number: "1022"
      })
    end
  end
end
```

**Step 8: Run tests**

```bash
cd server && mix test test/photo_finish/events/event_competitor_test.exs
```

**Step 9: Commit**

```bash
git add -A && git commit -m "refactor: split Competitor into Competitor + EventCompetitor

- Competitor: person record (first_name, last_name, external_id, email, phone)
- EventCompetitor: per-event participation (session, competitor_number, team, level)
- Photo now references event_competitor_id"
```

---

## Task 2: Build Roster Parser Module

**Files:**
- Create: `server/lib/photo_finish/events/roster_parser.ex`
- Test: `server/test/photo_finish/events/roster_parser_test.exs`

**Step 1: Write failing test**

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

      {:ok, competitors} = RosterParser.parse_txt(content)

      assert length(competitors) == 3
      assert Enum.at(competitors, 0) == %{
        competitor_number: "143",
        first_name: "Avery",
        last_name: "W"
      }
    end

    test "handles single-word names" do
      content = "143 Avery\n"

      {:ok, [competitor]} = RosterParser.parse_txt(content)

      assert competitor.first_name == "Avery"
      assert competitor.last_name == nil
    end

    test "skips blank lines" do
      content = "143 Avery W\n\n169 Callie W\n"

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

**Step 2: Implement parser**

Create `server/lib/photo_finish/events/roster_parser.ex`:

```elixir
defmodule PhotoFinish.Events.RosterParser do
  @moduledoc """
  Parses roster files into competitor data.
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
      {:ok, Enum.map(results, fn {:ok, c} -> c end)}
    else
      {:error, "Invalid lines found"}
    end
  end

  defp parse_line(line) do
    case String.split(line, " ", parts: 2) do
      [number, name] when byte_size(number) > 0 and byte_size(name) > 0 ->
        {first_name, last_name} = split_name(name)
        {:ok, %{competitor_number: number, first_name: first_name, last_name: last_name}}

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

**Step 3: Run tests**

```bash
cd server && mix test test/photo_finish/events/roster_parser_test.exs
```

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: add RosterParser for .txt roster files"
```

---

## Task 3: Build Roster Import Context Function

**Files:**
- Create: `server/lib/photo_finish/events/roster_import.ex`
- Test: `server/test/photo_finish/events/roster_import_test.exs`

**Step 1: Write failing test**

Create `server/test/photo_finish/events/roster_import_test.exs`:

```elixir
defmodule PhotoFinish.Events.RosterImportTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{RosterImport, Competitor, EventCompetitor}

  describe "import_roster/3" do
    test "creates competitor and event_competitor records" do
      {:ok, event} = create_test_event()

      content = """
      143 Avery W
      169 Callie W
      """

      {:ok, result} = RosterImport.import_roster(event.id, "3A", content)

      assert result.imported_count == 2

      # Verify both tables were populated
      competitors = Ash.read!(Competitor)
      event_competitors = Ash.read!(EventCompetitor)

      assert length(competitors) == 2
      assert length(event_competitors) == 2

      avery_ec = Enum.find(event_competitors, &(&1.competitor_number == "143"))
      assert avery_ec.session == "3A"
      assert avery_ec.display_name == "143 Avery W"
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

**Step 2: Implement import module**

Create `server/lib/photo_finish/events/roster_import.ex`:

```elixir
defmodule PhotoFinish.Events.RosterImport do
  @moduledoc """
  Imports competitor rosters from file content.
  Creates both Competitor (person) and EventCompetitor (event participation) records.
  """

  alias PhotoFinish.Events.{Competitor, EventCompetitor, RosterParser}

  def import_roster(event_id, session, content) when is_binary(content) do
    with {:ok, parsed} <- RosterParser.parse_txt(content) do
      results =
        Enum.map(parsed, fn data ->
          create_competitor_pair(event_id, session, data)
        end)

      imported = Enum.count(results, &match?({:ok, _}, &1))
      errors = Enum.count(results, &match?({:error, _}, &1))

      {:ok, %{imported_count: imported, error_count: errors}}
    end
  end

  defp create_competitor_pair(event_id, session, %{
         competitor_number: number,
         first_name: first,
         last_name: last
       }) do
    # Create the person record
    with {:ok, competitor} <- Ash.create(Competitor, %{
           first_name: first,
           last_name: last
         }) do
      # Create the event participation record
      display_name = build_display_name(number, first, last)

      Ash.create(EventCompetitor, %{
        competitor_id: competitor.id,
        event_id: event_id,
        session: session,
        competitor_number: number,
        display_name: display_name
      })
    end
  end

  defp build_display_name(number, first, nil), do: "#{number} #{first}"
  defp build_display_name(number, first, last), do: "#{number} #{first} #{last}"
end
```

**Step 3: Run tests**

```bash
cd server && mix test test/photo_finish/events/roster_import_test.exs
```

**Step 4: Commit**

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
live "/events/:event_id/competitors/import", Admin.CompetitorLive.Import, :import
```

**Step 2: Create Import LiveView**

Create `server/lib/photo_finish_web/live/admin/competitor_live/import.ex`:

```elixir
defmodule PhotoFinishWeb.Admin.CompetitorLive.Import do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Events.RosterImport
  alias PhotoFinish.Events.RosterParser

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(Event, event_id)

    socket =
      socket
      |> assign(:event, event)
      |> assign(:session, "")
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
            <label class="block text-sm font-medium text-gray-700 mb-2">Session</label>
            <input
              type="text"
              name="session"
              value={@session}
              placeholder="e.g. 3A, 11B"
              class="w-32 rounded-md border-gray-300 shadow-sm focus:border-indigo-500 focus:ring-indigo-500"
              required
            />
          </div>

          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">Roster File (.txt)</label>
            <.live_file_input upload={@uploads.roster} class="block w-full text-sm text-gray-500
              file:mr-4 file:py-2 file:px-4 file:rounded-md file:border-0
              file:text-sm file:font-semibold file:bg-indigo-50 file:text-indigo-700
              hover:file:bg-indigo-100" />
            <p class="mt-1 text-xs text-gray-500">Format: one competitor per line, "NUMBER NAME"</p>
          </div>

          <%= if @preview do %>
            <div class="border rounded-lg overflow-hidden">
              <div class="bg-gray-50 px-4 py-2 border-b">
                <span class="font-medium">Preview</span>
                <span class="text-gray-500 text-sm ml-2"><%= length(@preview) %> competitors</span>
              </div>
              <div class="max-h-64 overflow-y-auto">
                <table class="min-w-full divide-y divide-gray-200">
                  <thead class="bg-gray-50">
                    <tr>
                      <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">Number</th>
                      <th class="px-4 py-2 text-left text-xs font-medium text-gray-500">Name</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-gray-200">
                    <%= for c <- @preview do %>
                      <tr>
                        <td class="px-4 py-2 text-sm font-mono"><%= c.competitor_number %></td>
                        <td class="px-4 py-2 text-sm"><%= c.first_name %> <%= c.last_name %></td>
                      </tr>
                    <% end %>
                  </tbody>
                </table>
              </div>
            </div>
          <% end %>

          <%= if @result do %>
            <div class="rounded-lg p-4 bg-green-50 text-green-800">
              <p class="font-medium">Imported <%= @result.imported_count %> competitors</p>
            </div>
          <% end %>

          <button
            type="submit"
            disabled={@preview == nil or @session == ""}
            class="px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 disabled:opacity-50"
          >
            Import Competitors
          </button>
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
          content = consume_upload(socket, entry)
          case RosterParser.parse_txt(content) do
            {:ok, competitors} -> assign(socket, :preview, competitors)
            {:error, _} -> assign(socket, :preview, nil)
          end
        _ ->
          assign(socket, :preview, nil)
      end

    {:noreply, socket}
  end

  def handle_event("import", %{"session" => session}, socket) do
    {[entry], []} = uploaded_entries(socket, :roster)
    content = consume_upload(socket, entry)

    case RosterImport.import_roster(socket.assigns.event.id, session, content) do
      {:ok, result} ->
        socket =
          socket
          |> assign(:result, result)
          |> assign(:preview, nil)
        {:noreply, socket}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Import failed: #{reason}")}
    end
  end

  defp consume_upload(socket, entry) do
    consume_uploaded_entry(socket, entry, fn %{path: path} ->
      {:ok, File.read!(path)}
    end)
  end
end
```

**Step 3: Test manually**

```bash
cd server && mix phx.server
```

Navigate to `/admin/events/{id}/competitors/import`

**Step 4: Commit**

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

  alias PhotoFinish.Events.{FolderAssociation, Competitor, EventCompetitor}
  alias PhotoFinish.Photos.Photo

  describe "list_unassigned_folders/2" do
    test "returns folders with unassigned photos" do
      {:ok, event} = create_test_event()

      create_photo(event.id, nil, "Gym 01", "3A", "Group 3A", "Beam")
      create_photo(event.id, nil, "Gym 01", "3A", "Group 3A", "Beam")
      create_photo(event.id, nil, "Gym 02", "3A", "Group 3A", "Beam")

      folders = FolderAssociation.list_unassigned_folders(event.id, %{
        gym: "A",
        session: "3A",
        group_name: "Group 3A",
        apparatus: "Beam"
      })

      assert length(folders) == 2
    end
  end

  describe "list_session_event_competitors/2" do
    test "returns event_competitors for session" do
      {:ok, event} = create_test_event()
      {:ok, ec1} = create_event_competitor(event.id, "3A", "1022")
      {:ok, ec2} = create_event_competitor(event.id, "3A", "1023")
      {:ok, _ec3} = create_event_competitor(event.id, "4A", "1024")

      result = FolderAssociation.list_session_event_competitors(event.id, "3A")

      assert length(result) == 2
    end
  end

  describe "assign_folder/3" do
    test "sets event_competitor_id on photos in folder" do
      {:ok, event} = create_test_event()
      {:ok, ec} = create_event_competitor(event.id, "3A", "1022")
      {:ok, p1} = create_photo(event.id, nil, "Gym 01", "3A", "Group 3A", "Beam")
      {:ok, p2} = create_photo(event.id, nil, "Gym 01", "3A", "Group 3A", "Beam")

      {:ok, count} = FolderAssociation.assign_folder(event.id, "Gym 01", ec.id)

      assert count == 2

      updated = Ash.get!(Photo, p1.id)
      assert updated.event_competitor_id == ec.id
    end
  end

  # Helper functions...
end
```

**Step 2: Implement FolderAssociation**

Create `server/lib/photo_finish/events/folder_association.ex`:

```elixir
defmodule PhotoFinish.Events.FolderAssociation do
  import Ecto.Query
  alias PhotoFinish.Repo
  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Events.EventCompetitor

  def list_unassigned_folders(event_id, %{gym: gym, session: session, group_name: group_name, apparatus: apparatus}) do
    from(p in Photo,
      where: p.event_id == ^event_id,
      where: p.gym == ^gym,
      where: p.session == ^session,
      where: p.group_name == ^group_name,
      where: p.apparatus == ^apparatus,
      where: is_nil(p.event_competitor_id),
      where: not is_nil(p.source_folder),
      group_by: p.source_folder,
      select: %{source_folder: p.source_folder, photo_count: count(p.id)},
      order_by: p.source_folder
    )
    |> Repo.all()
  end

  def list_session_event_competitors(event_id, session) do
    EventCompetitor
    |> Ash.Query.filter(event_id == ^event_id and session == ^session)
    |> Ash.Query.sort(:competitor_number)
    |> Ash.read!()
  end

  def assign_folder(event_id, source_folder, event_competitor_id) do
    {count, _} =
      from(p in Photo,
        where: p.event_id == ^event_id,
        where: p.source_folder == ^source_folder,
        where: is_nil(p.event_competitor_id)
      )
      |> Repo.update_all(set: [event_competitor_id: event_competitor_id, updated_at: DateTime.utc_now()])

    {:ok, count}
  end
end
```

**Step 3: Run tests and commit**

```bash
cd server && mix test test/photo_finish/events/folder_association_test.exs
git add -A && git commit -m "feat: add FolderAssociation for linking photos to event_competitors"
```

---

## Task 6: Build Folder Association LiveView UI

**Files:**
- Create: `server/lib/photo_finish_web/live/admin/folder_live/associate.ex`
- Modify: `server/lib/photo_finish_web/router.ex`

Similar to previous plan but using `event_competitor_id` instead of `competitor_id`.

---

## Task 7: Build Viewer Search Context Function

**Files:**
- Create: `server/lib/photo_finish/viewer/search.ex`
- Test: `server/test/photo_finish/viewer/search_test.exs`

Search queries `event_competitors` table joined with `competitors` for name matching.

---

## Task 8: Build Viewer Home LiveView (Search)

Same as before.

---

## Task 9: Build Viewer Competitor Photo Grid

Uses `event_competitor_id` for photo loading.

---

## Task 10: Add Public Photo Serving Routes

Same as before.

---

## Summary

The key difference from the incorrect plan:

1. **Two tables**: `competitors` (person) + `event_competitors` (per-event participation with session)
2. **Photos reference** `event_competitor_id` not `competitor_id`
3. **Import creates both**: a Competitor record AND an EventCompetitor record
4. **Search queries** `event_competitors` joined with `competitors`
