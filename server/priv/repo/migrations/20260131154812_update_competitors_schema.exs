defmodule PhotoFinish.Repo.Migrations.UpdateCompetitorsSchema do
  @moduledoc """
  Updates competitors table:
  - Changes id from UUID to string (for prefixed IDs like cmp_abc1234)
  - Updates photos.competitor_id foreign key column type to text
  """

  use Ecto.Migration

  def up do
    # Drop foreign key constraints that reference competitors.id
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_competitor_id_fkey"

    # Change the competitors.id column from uuid to text
    execute "ALTER TABLE competitors ALTER COLUMN id DROP DEFAULT"
    execute "ALTER TABLE competitors ALTER COLUMN id TYPE text USING id::text"

    # Change the photos.competitor_id foreign key column from uuid to text
    execute "ALTER TABLE photos ALTER COLUMN competitor_id TYPE text USING competitor_id::text"

    # Re-add foreign key constraint
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_competitor_id_fkey
    FOREIGN KEY (competitor_id) REFERENCES competitors(id)
    """
  end

  def down do
    # Drop foreign key constraint
    execute "ALTER TABLE photos DROP CONSTRAINT IF EXISTS photos_competitor_id_fkey"

    # Change id columns back to uuid
    execute "ALTER TABLE competitors ALTER COLUMN id TYPE uuid USING id::uuid"
    execute "ALTER TABLE competitors ALTER COLUMN id SET DEFAULT gen_random_uuid()"
    execute "ALTER TABLE photos ALTER COLUMN competitor_id TYPE uuid USING competitor_id::uuid"

    # Re-add foreign key constraint with uuid type
    execute """
    ALTER TABLE photos
    ADD CONSTRAINT photos_competitor_id_fkey
    FOREIGN KEY (competitor_id) REFERENCES competitors(id)
    """
  end
end
