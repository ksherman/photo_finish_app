defmodule PhotoFinish.Repo.Migrations.UpdateEventsSchema do
  @moduledoc """
  Updates events table:
  - Changes id from UUID to string (for prefixed IDs like evt_abc1234)
  - Replaces storage_directory with storage_root (required)
  - Adds num_gyms and sessions_per_gym configuration fields
  """

  use Ecto.Migration

  def up do
    # Drop foreign key constraints that reference events.id
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_event_id_fkey"
    execute "ALTER TABLE competitors DROP CONSTRAINT IF EXISTS competitors_event_id_fkey"
    execute "ALTER TABLE hierarchy_nodes DROP CONSTRAINT IF EXISTS hierarchy_nodes_event_id_fkey"
    execute "ALTER TABLE hierarchy_levels DROP CONSTRAINT IF EXISTS hierarchy_levels_event_id_fkey"

    # Change the events.id column from uuid to text
    execute "ALTER TABLE events ALTER COLUMN id DROP DEFAULT"
    execute "ALTER TABLE events ALTER COLUMN id TYPE text USING id::text"

    # Change the foreign key columns from uuid to text
    execute "ALTER TABLE photos ALTER COLUMN event_id TYPE text USING event_id::text"
    execute "ALTER TABLE competitors ALTER COLUMN event_id TYPE text USING event_id::text"
    execute "ALTER TABLE hierarchy_nodes ALTER COLUMN event_id TYPE text USING event_id::text"
    execute "ALTER TABLE hierarchy_levels ALTER COLUMN event_id TYPE text USING event_id::text"

    # Re-add foreign key constraints
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE competitors
    ADD CONSTRAINT competitors_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE hierarchy_nodes
    ADD CONSTRAINT hierarchy_nodes_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE hierarchy_levels
    ADD CONSTRAINT hierarchy_levels_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    # Replace storage_directory with storage_root
    alter table(:events) do
      add :storage_root, :text, null: false, default: ""
      add :num_gyms, :integer, null: false, default: 1
      add :sessions_per_gym, :integer, null: false, default: 1
    end

    # Copy existing data from storage_directory to storage_root
    execute "UPDATE events SET storage_root = COALESCE(storage_directory, '')"

    # Remove storage_directory
    alter table(:events) do
      remove :storage_directory
    end

    # Remove the default on storage_root (it was just for migration)
    execute "ALTER TABLE events ALTER COLUMN storage_root DROP DEFAULT"
  end

  def down do
    # Add back storage_directory
    alter table(:events) do
      add :storage_directory, :text
    end

    # Copy data back
    execute "UPDATE events SET storage_directory = storage_root"

    # Remove new columns
    alter table(:events) do
      remove :storage_root
      remove :num_gyms
      remove :sessions_per_gym
    end

    # Drop foreign key constraints
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_event_id_fkey"
    execute "ALTER TABLE competitors DROP CONSTRAINT IF EXISTS competitors_event_id_fkey"
    execute "ALTER TABLE hierarchy_nodes DROP CONSTRAINT IF EXISTS hierarchy_nodes_event_id_fkey"
    execute "ALTER TABLE hierarchy_levels DROP CONSTRAINT IF EXISTS hierarchy_levels_event_id_fkey"

    # Change id columns back to uuid
    execute "ALTER TABLE events ALTER COLUMN id TYPE uuid USING id::uuid"
    execute "ALTER TABLE events ALTER COLUMN id SET DEFAULT gen_random_uuid()"
    execute "ALTER TABLE photos ALTER COLUMN event_id TYPE uuid USING event_id::uuid"
    execute "ALTER TABLE competitors ALTER COLUMN event_id TYPE uuid USING event_id::uuid"
    execute "ALTER TABLE hierarchy_nodes ALTER COLUMN event_id TYPE uuid USING event_id::uuid"
    execute "ALTER TABLE hierarchy_levels ALTER COLUMN event_id TYPE uuid USING event_id::uuid"

    # Re-add foreign key constraints with uuid type
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE competitors
    ADD CONSTRAINT competitors_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE hierarchy_nodes
    ADD CONSTRAINT hierarchy_nodes_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE hierarchy_levels
    ADD CONSTRAINT hierarchy_levels_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """
  end
end
