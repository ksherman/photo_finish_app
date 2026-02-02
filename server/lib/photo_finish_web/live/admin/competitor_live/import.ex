defmodule PhotoFinishWeb.Admin.CompetitorLive.Import do
  @moduledoc """
  LiveView for importing competitor rosters from text files.

  Allows admins to:
  1. Enter a session name (e.g., "3A")
  2. Upload a .txt roster file
  3. Preview the parsed competitors
  4. Import them (creates Competitor + EventCompetitor records)
  """
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Events.RosterImport
  alias PhotoFinish.Events.RosterParser

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(Event, event_id)

    socket =
      socket
      |> assign(:page_title, "Import Roster")
      |> assign(:event, event)
      |> assign(:session, "")
      |> assign(:preview, nil)
      |> assign(:file_content, nil)
      |> assign(:result, nil)
      |> allow_upload(:roster, accept: ~w(.txt), max_entries: 1)

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="min-h-screen bg-gray-50">
        <%!-- Header --%>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <div class="max-w-2xl mx-auto">
            <div class="flex items-center gap-4">
              <.button_link
                navigate={~p"/admin/events/#{@event.id}"}
                variant="outline"
                color="natural"
                size="small"
              >
                <.icon name="hero-arrow-left" class="w-4 h-4" />
              </.button_link>
              <div>
                <h1 class="text-xl font-bold text-gray-900">Import Roster</h1>
                <p class="text-sm text-gray-500">{@event.name}</p>
              </div>
            </div>
          </div>
        </div>

        <%!-- Main Content --%>
        <div class="max-w-2xl mx-auto py-8 px-6">
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Upload Roster File</h2>
              <p class="text-sm text-gray-500 mt-1">
                Import competitors from a text file. Format: one competitor per line, "NUMBER NAME"
              </p>
            </div>

            <div class="p-6">
              <form phx-submit="import" phx-change="validate" class="space-y-6">
                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-2">Session</label>
                  <input
                    type="text"
                    name="session"
                    value={@session}
                    placeholder="e.g., 3A, 11B"
                    class="w-32 rounded-md border-gray-300 shadow-sm focus:border-indigo-500 focus:ring-indigo-500"
                    required
                  />
                  <p class="mt-1 text-xs text-gray-500">
                    The session identifier for these competitors
                  </p>
                </div>

                <div>
                  <label class="block text-sm font-medium text-gray-700 mb-2">
                    Roster File (.txt)
                  </label>
                  <.live_file_input
                    upload={@uploads.roster}
                    class="block w-full text-sm text-gray-500
                      file:mr-4 file:py-2 file:px-4 file:rounded-md file:border-0
                      file:text-sm file:font-semibold file:bg-indigo-50 file:text-indigo-700
                      hover:file:bg-indigo-100"
                  />
                  <p class="mt-1 text-xs text-gray-500">
                    Format: one competitor per line, "NUMBER NAME" (e.g., "123 Jane Doe")
                  </p>

                  <%!-- Upload errors --%>
                  <%= for entry <- @uploads.roster.entries do %>
                    <%= for err <- upload_errors(@uploads.roster, entry) do %>
                      <p class="mt-2 text-sm text-red-600">
                        {error_to_string(err)}
                      </p>
                    <% end %>
                  <% end %>
                </div>

                <%!-- Preview --%>
                <%= if @preview do %>
                  <div class="border rounded-lg overflow-hidden">
                    <div class="bg-gray-50 px-4 py-2 border-b flex items-center justify-between">
                      <div>
                        <span class="font-medium">Preview</span>
                        <span class="text-gray-500 text-sm ml-2">
                          {length(@preview)} competitors
                        </span>
                      </div>
                      <.icon name="hero-check-circle" class="w-5 h-5 text-green-500" />
                    </div>
                    <div class="max-h-64 overflow-y-auto">
                      <table class="min-w-full divide-y divide-gray-200">
                        <thead class="bg-gray-50">
                          <tr>
                            <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                              Number
                            </th>
                            <th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                              Name
                            </th>
                          </tr>
                        </thead>
                        <tbody class="bg-white divide-y divide-gray-200">
                          <%= for c <- @preview do %>
                            <tr>
                              <td class="px-4 py-2 text-sm font-mono text-gray-900">
                                {c.competitor_number}
                              </td>
                              <td class="px-4 py-2 text-sm text-gray-900">
                                {c.first_name} {c.last_name}
                              </td>
                            </tr>
                          <% end %>
                        </tbody>
                      </table>
                    </div>
                  </div>
                <% end %>

                <%!-- Result --%>
                <%= if @result do %>
                  <div class="rounded-lg p-4 bg-green-50 border border-green-200">
                    <div class="flex items-center gap-2">
                      <.icon name="hero-check-circle" class="w-5 h-5 text-green-600" />
                      <p class="font-medium text-green-800">
                        Successfully imported {@result.imported_count} competitors
                      </p>
                    </div>
                    <%= if @result.error_count > 0 do %>
                      <p class="text-sm text-green-700 mt-1">
                        {@result.error_count} entries could not be imported
                      </p>
                    <% end %>
                  </div>
                <% end %>

                <div class="flex items-center gap-3 pt-4 border-t border-gray-200">
                  <.button
                    type="submit"
                    disabled={@preview == nil or @session == ""}
                    size="medium"
                    color="primary"
                  >
                    <.icon name="hero-arrow-up-tray" class="w-4 h-4 mr-2" />
                    Import Competitors
                  </.button>

                  <.button_link
                    navigate={~p"/admin/events/#{@event.id}"}
                    variant="outline"
                    color="natural"
                    size="medium"
                  >
                    Cancel
                  </.button_link>
                </div>
              </form>
            </div>
          </div>

          <%!-- Help Section --%>
          <div class="mt-6 bg-blue-50 rounded-lg border border-blue-100 p-4">
            <h3 class="text-sm font-semibold text-blue-900 flex items-center gap-2">
              <.icon name="hero-information-circle" class="w-4 h-4" />
              File Format
            </h3>
            <div class="mt-2 text-sm text-blue-800">
              <p>Each line should contain a competitor number followed by their name:</p>
              <pre class="mt-2 bg-blue-100 rounded p-2 font-mono text-xs">{"123 Jane Doe\n456 John Smith\n789 Alex Johnson"}</pre>
            </div>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def handle_event("validate", %{"session" => session}, socket) do
    socket = assign(socket, :session, session)

    # Parse uploaded file for preview (if any valid entries)
    socket =
      case uploaded_entries(socket, :roster) do
        {[_entry], []} ->
          # Read file content and parse for preview
          {content, socket} = read_upload_content(socket)

          case RosterParser.parse_txt(content) do
            {:ok, competitors} ->
              socket
              |> assign(:preview, competitors)
              |> assign(:file_content, content)

            {:error, _} ->
              socket
              |> assign(:preview, nil)
              |> assign(:file_content, nil)
          end

        _ ->
          socket
          |> assign(:preview, nil)
          |> assign(:file_content, nil)
      end

    {:noreply, socket}
  end

  def handle_event("import", %{"session" => session}, socket) do
    # Use stored file content if available, otherwise try to read again
    content = socket.assigns.file_content

    if is_nil(content) or content == "" do
      {:noreply, put_flash(socket, :error, "Please upload a roster file")}
    else
      case RosterImport.import_roster(socket.assigns.event.id, session, content) do
        {:ok, result} ->
          socket =
            socket
            |> assign(:result, result)
            |> assign(:preview, nil)
            |> assign(:file_content, nil)
            |> put_flash(:info, "Successfully imported #{result.imported_count} competitors")

          {:noreply, socket}

        {:error, reason} ->
          {:noreply, put_flash(socket, :error, "Import failed: #{inspect(reason)}")}
      end
    end
  end

  # Read upload content during validation
  # Note: We use consume_uploaded_entries which works during phx-change
  defp read_upload_content(socket) do
    results =
      consume_uploaded_entries(socket, :roster, fn %{path: path}, _entry ->
        {:ok, File.read!(path)}
      end)

    content =
      case results do
        [content] -> content
        _ -> ""
      end

    {content, socket}
  end

  defp error_to_string(:too_large), do: "File is too large"
  defp error_to_string(:not_accepted), do: "File type not accepted. Please upload a .txt file"
  defp error_to_string(:too_many_files), do: "Too many files"
  defp error_to_string(err), do: "Error: #{inspect(err)}"
end
