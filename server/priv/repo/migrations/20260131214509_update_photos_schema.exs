defmodule PhotoFinish.Repo.Migrations.UpdatePhotosSchema do
  @moduledoc """
  Updates photos table:
  - Changes id from UUID to string (for prefixed IDs like pho_abc1234)
  - Adds flat location fields (gym, session, group_name, apparatus)
  """

  use Ecto.Migration

  def up do
    # Drop foreign key constraints that reference photos.id (if any)
    # Currently none, but future-proofing

    # Drop foreign key from photos to events temporarily
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_event_id_fkey"
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_competitor_id_fkey"

    # Change the photos.id column from uuid to text
    execute "ALTER TABLE photos ALTER COLUMN id DROP DEFAULT"
    execute "ALTER TABLE photos ALTER COLUMN id TYPE text USING id::text"

    # Re-add foreign key constraints
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_competitor_id_fkey
    FOREIGN KEY (competitor_id) REFERENCES competitors(id)
    """

    # Add location fields
    alter table(:photos) do
      add :gym, :text
      add :session, :text
      add :group_name, :text
      add :apparatus, :text
    end
  end

  def down do
    # Remove location fields
    alter table(:photos) do
      remove :gym
      remove :session
      remove :group_name
      remove :apparatus
    end

    # Drop foreign key constraints
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_event_id_fkey"
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_competitor_id_fkey"

    # Change id back to uuid
    execute "ALTER TABLE photos ALTER COLUMN id TYPE uuid USING id::uuid"
    execute "ALTER TABLE photos ALTER COLUMN id SET DEFAULT gen_random_uuid()"

    # Re-add foreign key constraints
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_event_id_fkey
    FOREIGN KEY (event_id) REFERENCES events(id)
    """

    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_competitor_id_fkey
    FOREIGN KEY (competitor_id) REFERENCES competitors(id)
    """
  end
end
