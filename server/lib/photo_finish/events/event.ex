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
        :storage_root,
        :num_gyms,
        :sessions_per_gym
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
        :storage_root,
        :num_gyms,
        :sessions_per_gym
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.event_id/0
      writable? false
    end

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
      constraints one_of: [:active, :archived]
    end

    attribute :order_code, :string do
      public? true
    end

    attribute :tax_rate_basis_points, :integer do
      public? true
      default 850
    end

    attribute :storage_root, :string do
      allow_nil? false
      public? true
    end

    attribute :num_gyms, :integer do
      allow_nil? false
      default 1
      public? true
    end

    attribute :sessions_per_gym, :integer do
      allow_nil? false
      default 1
      public? true
    end

    timestamps()
  end

  relationships do
    has_many :competitors, PhotoFinish.Events.Competitor do
      public? true
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      public? true
    end
  end
end
