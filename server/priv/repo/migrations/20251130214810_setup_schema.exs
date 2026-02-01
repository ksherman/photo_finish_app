defmodule PhotoFinish.Repo.Migrations.SetupSchema do
  @moduledoc """
  Creates the complete PhotoFinish database schema.

  Tables:
  - users: Admin/staff accounts with authentication
  - tokens: JWT tokens for authentication
  - api_keys: API keys for programmatic access
  - events: Sporting events (gymnastics meets, etc.)
  - competitors: Athletes/participants at events
  - photos: Photos taken at events

  All domain entities (events, competitors, photos) use string IDs with
  prefixes (evt_, cmp_, pho_) for easy identification.
  """

  use Ecto.Migration

  def up do
    # Users table (UUID primary key)
    create table(:users, primary_key: false) do
      add :id, :uuid, null: false, default: fragment("gen_random_uuid()"), primary_key: true
      add :email, :citext, null: false
      add :hashed_password, :text, null: false
      add :confirmed_at, :utc_datetime_usec
    end

    create unique_index(:users, [:email], name: "users_unique_email_index")

    # Tokens table (JTI as primary key)
    create table(:tokens, primary_key: false) do
      add :jti, :text, null: false, primary_key: true
      add :subject, :text, null: false
      add :expires_at, :utc_datetime, null: false
      add :purpose, :text, null: false
      add :extra_data, :map

      add :created_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :inserted_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :updated_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")
    end

    # API keys table
    create table(:api_keys, primary_key: false) do
      add :id, :uuid, null: false, default: fragment("gen_random_uuid()"), primary_key: true
      add :api_key_hash, :binary, null: false
      add :expires_at, :utc_datetime_usec, null: false

      add :user_id,
          references(:users,
            column: :id,
            name: "api_keys_user_id_fkey",
            type: :uuid,
            prefix: "public"
          )
    end

    create unique_index(:api_keys, [:api_key_hash], name: "api_keys_unique_api_key_index")

    # Events table (string ID with evt_ prefix)
    create table(:events, primary_key: false) do
      add :id, :text, null: false, primary_key: true
      add :name, :text, null: false
      add :slug, :text, null: false
      add :description, :text
      add :starts_at, :utc_datetime
      add :ends_at, :utc_datetime
      add :status, :text, default: "active"
      add :order_code, :text
      add :tax_rate_basis_points, :bigint, default: 850
      add :storage_root, :text, null: false
      add :num_gyms, :integer, null: false, default: 1
      add :sessions_per_gym, :integer, null: false, default: 1

      add :inserted_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :updated_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")
    end

    # Competitors table (string ID with cmp_ prefix)
    create table(:competitors, primary_key: false) do
      add :id, :text, null: false, primary_key: true
      add :competitor_number, :text, null: false
      add :first_name, :text, null: false
      add :last_name, :text
      add :display_name, :text
      add :team_name, :text
      add :level, :text
      add :age_group, :text
      add :email, :text
      add :phone, :text
      add :is_active, :boolean, default: true
      add :metadata, :map

      add :inserted_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :updated_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :event_id,
          references(:events,
            column: :id,
            name: "competitors_event_id_fkey",
            type: :text,
            prefix: "public"
          )
    end

    # Photos table (string ID with pho_ prefix)
    create table(:photos, primary_key: false) do
      add :id, :text, null: false, primary_key: true
      add :ingestion_path, :text, null: false
      add :current_path, :text
      add :preview_path, :text
      add :thumbnail_path, :text
      add :filename, :text, null: false
      add :original_filename, :text
      add :file_size_bytes, :bigint
      add :mime_type, :text, default: "image/jpeg"
      add :width, :bigint
      add :height, :bigint
      add :photographer, :text
      add :source_folder, :text
      add :captured_at, :utc_datetime
      add :processed_at, :utc_datetime
      add :finalized_at, :utc_datetime
      add :status, :text, default: "discovered"
      add :error_message, :text
      add :exif_data, :map
      add :metadata, :map

      # Location fields (parsed from folder structure)
      add :gym, :text
      add :session, :text
      add :group_name, :text
      add :apparatus, :text

      add :inserted_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :updated_at, :utc_datetime_usec,
        null: false,
        default: fragment("(now() AT TIME ZONE 'utc')")

      add :event_id,
          references(:events,
            column: :id,
            name: "photos_event_id_fkey",
            type: :text,
            prefix: "public"
          )

      add :competitor_id,
          references(:competitors,
            column: :id,
            name: "photos_competitor_id_fkey",
            type: :text,
            prefix: "public"
          )
    end
  end

  def down do
    drop table(:photos)
    drop table(:competitors)
    drop table(:events)
    drop_if_exists unique_index(:api_keys, [:api_key_hash], name: "api_keys_unique_api_key_index")
    drop constraint(:api_keys, "api_keys_user_id_fkey")
    drop table(:api_keys)
    drop table(:tokens)
    drop_if_exists unique_index(:users, [:email], name: "users_unique_email_index")
    drop table(:users)
  end
end
