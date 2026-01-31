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
      create: [
        :event_id,
        :competitor_number,
        :first_name,
        :last_name,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :email,
        :phone,
        :is_active,
        :metadata
      ],
      update: [
        :event_id,
        :competitor_number,
        :first_name,
        :last_name,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :email,
        :phone,
        :is_active,
        :metadata
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.competitor_id/0
      writable? false
    end

    attribute :competitor_number, :string do
      allow_nil? false
      public? true
    end

    attribute :first_name, :string do
      allow_nil? false
      public? true
    end

    attribute :last_name, :string do
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

    attribute :email, :string do
      public? true
    end

    attribute :phone, :string do
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
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      public? true
    end
  end
end
