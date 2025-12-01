defmodule PhotoFinish.Events.Event do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "events"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :name,
        :slug,
        :description,
        :starts_at,
        :ends_at,
        :status,
        :order_code,
        :tax_rate_basis_points,
        :storage_directory
      ],
      update: [
        :name,
        :slug,
        :description,
        :starts_at,
        :ends_at,
        :status,
        :order_code,
        :tax_rate_basis_points,
        :storage_directory
      ]
    ]
  end

  attributes do
    uuid_primary_key :id

    attribute :name, :string do
      allow_nil? false
      public? true
    end

    attribute :slug, :string do
      allow_nil? false
      public? true
    end

    attribute :description, :string do
      public? true
    end

    attribute :starts_at, :utc_datetime do
      public? true
    end

    attribute :ends_at, :utc_datetime do
      public? true
    end

    attribute :status, :atom do
      public? true
      default :active
      constraints [one_of: [:active, :archived]]
    end

    attribute :order_code, :string do
      public? true
    end

    attribute :tax_rate_basis_points, :integer do
      public? true
      default 850
    end

    attribute :storage_directory, :string do
      public? true
    end

    timestamps()
  end

  relationships do
    has_many :hierarchy_levels, PhotoFinish.Events.HierarchyLevel do
      public? true
    end

    has_many :hierarchy_nodes, PhotoFinish.Events.HierarchyNode do
      public? true
    end

    has_many :competitors, PhotoFinish.Events.Competitor do
      public? true
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      public? true
    end
  end
end