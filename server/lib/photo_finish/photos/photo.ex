defmodule PhotoFinish.Photos.Photo do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Photos,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "photos"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :event_id,
        :competitor_id,
        :ingestion_path,
        :current_path,
        :preview_path,
        :thumbnail_path,
        :filename,
        :original_filename,
        :file_size_bytes,
        :mime_type,
        :width,
        :height,
        :photographer,
        :source_folder,
        :captured_at,
        :status,
        :error_message,
        :exif_data,
        :metadata,
        :gym,
        :session,
        :group_name,
        :apparatus
      ],
      update: [
        :event_id,
        :competitor_id,
        :ingestion_path,
        :current_path,
        :preview_path,
        :thumbnail_path,
        :filename,
        :original_filename,
        :file_size_bytes,
        :mime_type,
        :width,
        :height,
        :photographer,
        :source_folder,
        :captured_at,
        :processed_at,
        :status,
        :error_message,
        :exif_data,
        :metadata,
        :gym,
        :session,
        :group_name,
        :apparatus
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.photo_id/0
      writable? false
    end

    attribute :ingestion_path, :string do
      allow_nil? false
      public? true
    end

    attribute :current_path, :string do
      public? true
    end

    attribute :preview_path, :string do
      public? true
    end

    attribute :thumbnail_path, :string do
      public? true
    end

    attribute :filename, :string do
      allow_nil? false
      public? true
    end

    attribute :original_filename, :string do
      public? true
    end

    attribute :file_size_bytes, :integer do
      public? true
    end

    attribute :mime_type, :string do
      public? true
      default "image/jpeg"
    end

    attribute :width, :integer do
      public? true
    end

    attribute :height, :integer do
      public? true
    end

    attribute :photographer, :string do
      public? true
    end

    attribute :source_folder, :string do
      public? true
    end

    attribute :captured_at, :utc_datetime do
      public? true
    end

    attribute :processed_at, :utc_datetime do
      public? true
    end

    attribute :finalized_at, :utc_datetime do
      public? true
    end

    attribute :status, :atom do
      public? true
      default :discovered
      constraints one_of: [:discovered, :processing, :ready, :finalized, :error, :duplicate]
    end

    attribute :error_message, :string do
      public? true
    end

    attribute :exif_data, :map do
      public? true
    end

    attribute :metadata, :map do
      public? true
    end

    # Location fields (flat, parsed from folder structure)
    attribute :gym, :string do
      public? true
    end

    attribute :session, :string do
      public? true
    end

    attribute :group_name, :string do
      public? true
    end

    attribute :apparatus, :string do
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    belongs_to :competitor, PhotoFinish.Events.Competitor do
      public? true
      attribute_type :string
    end
  end
end
