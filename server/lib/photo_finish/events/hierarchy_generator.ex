defmodule PhotoFinish.Events.HierarchyGenerator do
  @moduledoc """
  Generates hierarchy levels, nodes, and folder structure for events
  """

  require Logger
  alias PhotoFinish.Events.HierarchyNode

  @doc """
  Generates the complete hierarchy structure for an event based on the provided configuration.

  Returns `{:ok, stats}` with statistics about what was created, or `{:error, reason}`.

  ## Examples

      iex> config = [
      ...>   %{level_name: "Gym", count: 2, naming_pattern: "alpha", add_rotations: false, allow_photos: false},
      ...>   %{level_name: "Session", count: 3, naming_pattern: "numeric", add_rotations: true, allow_photos: false},
      ...>   %{level_name: "Competitor", count: 0, naming_pattern: "numeric", add_rotations: false, allow_photos: true}
      ...> ]
      iex> generate_hierarchy(event, config)
      {:ok, %{levels_created: 3, nodes_created: 14, folders_created: 14}}
  """
  def generate_hierarchy(event, level_configs, create_folders? \\ true) do
    # Load the event with existing hierarchy levels
    event = Ash.load!(event, [:hierarchy_levels])

    Logger.info("Generating hierarchy for event #{event.id}")
    Logger.info("Create folders? #{create_folders?}, Storage dir: #{inspect(event.storage_directory)}")
    Logger.info("Level configs received: #{inspect(level_configs)}")
    Logger.info("Existing hierarchy levels: #{length(event.hierarchy_levels)}")

    # Use existing hierarchy levels from database
    existing_levels = Enum.sort_by(event.hierarchy_levels, & &1.level_number)

    # Validate we have the right number of level configs
    if length(level_configs) > length(existing_levels) do
      raise "Cannot generate more levels than exist in the database. " <>
              "Have #{length(existing_levels)} levels, but #{length(level_configs)} configs provided."
    end

    Logger.info("Using #{length(existing_levels)} existing hierarchy levels")

    # Step 1: Generate all hierarchy nodes recursively
    {:ok, nodes, node_count} = generate_nodes(event.id, existing_levels, level_configs)
    Logger.info("Generated #{node_count} hierarchy nodes")

    # Step 2: Create folder structure on disk if requested
    folders_created =
      if create_folders? && event.storage_directory do
        Logger.info("Creating folder structure at: #{event.storage_directory}")
        count = create_folder_structure(event.storage_directory, nodes)
        Logger.info("Created #{count} folders")
        count
      else
        Logger.warning("Skipping folder creation - create_folders?: #{create_folders?}, storage_directory: #{inspect(event.storage_directory)}")
        0
      end

    {:ok,
     %{
       levels_created: 0,
       nodes_created: node_count,
       folders_created: folders_created
     }}
  rescue
    error ->
      Logger.error("Error generating hierarchy: #{Exception.message(error)}")
      Logger.error("Stacktrace: #{inspect(__STACKTRACE__)}")
      {:error, Exception.message(error)}
  end

  # Private Functions

  defp generate_nodes(event_id, _levels, level_configs) do
    # Start recursive generation from root level
    {nodes, count} = generate_nodes_recursive(event_id, level_configs, nil, 1, [], 0)
    {:ok, nodes, count}
  end

  defp generate_nodes_recursive(_event_id, level_configs, _parent_id, level_idx, nodes, count)
       when level_idx > length(level_configs) do
    # Base case: we've processed all configured levels
    {nodes, count}
  end

  defp generate_nodes_recursive(event_id, level_configs, parent_id, level_idx, nodes, count) do
    config = Enum.at(level_configs, level_idx - 1)

    # If no config for this level, we're done (partial hierarchy generation)
    if config == nil do
      {nodes, count}
    else
      # Skip levels with count = 0 (these are for competitor folders that will be created dynamically)
      if config.count == 0 do
        {nodes, count}
      else
        # Generate names for this level
        names = generate_names(config)

        # Create nodes for this level and recurse for children
        {new_nodes, new_count} =
          Enum.reduce(names, {nodes, count}, fn {name, display_order}, {acc_nodes, acc_count} ->
            node_attrs = %{
              event_id: event_id,
              parent_id: parent_id,
              level_number: level_idx,
              name: name,
              slug: slugify(name),
              display_order: display_order
            }

            node = Ash.create!(HierarchyNode, node_attrs)

            # Recurse to create children
            {child_nodes, child_count} =
              generate_nodes_recursive(event_id, level_configs, node.id, level_idx + 1, [], 0)

            {acc_nodes ++ [node] ++ child_nodes, acc_count + 1 + child_count}
          end)

        {new_nodes, new_count}
      end
    end
  end

  defp generate_names(config) do
    base_names =
      case config.naming_pattern do
        "numeric" ->
          Enum.map(1..config.count, &to_string/1)

        "alpha" ->
          Enum.map(1..config.count, &int_to_alpha/1)

        "custom" ->
          prefix = config.custom_prefix || config.level_name
          Enum.map(1..config.count, &"#{prefix} #{&1}")

        _ ->
          Enum.map(1..config.count, &to_string/1)
      end

    # Apply A/B rotations if requested
    names_with_rotations =
      if config.add_rotations do
        Enum.flat_map(base_names, fn name -> ["#{name}A", "#{name}B"] end)
      else
        base_names
      end

    # Return with display order
    Enum.with_index(names_with_rotations)
  end

  defp int_to_alpha(n) when n > 0 and n <= 26 do
    <<n + 64>>
  end

  defp int_to_alpha(n) when n > 26 do
    # For numbers > 26, use AA, AB, etc.
    first = div(n - 1, 26)
    second = rem(n - 1, 26) + 1
    "#{int_to_alpha(first)}#{int_to_alpha(second)}"
  end

  defp slugify(name) do
    name
    |> String.downcase()
    |> String.replace(~r/[^\w\s-]/, "")
    |> String.replace(~r/\s+/, "-")
  end

  defp create_folder_structure(base_path, nodes) do
    # Group nodes by their hierarchy path
    nodes_with_paths = build_node_paths(nodes)
    Logger.info("Building #{map_size(nodes_with_paths)} folder paths")

    # Create each directory
    Enum.reduce(nodes_with_paths, 0, fn {_node_id, path}, acc ->
      full_path = Path.join([base_path | path])

      case File.mkdir_p(full_path) do
        :ok ->
          Logger.debug("Created folder: #{full_path}")
          acc + 1

        {:error, reason} ->
          Logger.error("Failed to create folder #{full_path}: #{inspect(reason)}")
          acc
      end
    end)
  end

  defp build_node_paths(nodes) do
    # Create a map of node_id -> node for quick lookup
    node_map = Map.new(nodes, fn node -> {node.id, node} end)

    # Build path for each node
    Map.new(nodes, fn node ->
      path = build_path_for_node(node, node_map, [])
      {node.id, path}
    end)
  end

  defp build_path_for_node(node, node_map, acc) do
    path = [node.name | acc]

    case node.parent_id do
      nil ->
        path

      parent_id ->
        parent = Map.get(node_map, parent_id)
        if parent, do: build_path_for_node(parent, node_map, path), else: path
    end
  end
end
