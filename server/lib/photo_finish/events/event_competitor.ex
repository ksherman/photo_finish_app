defmodule PhotoFinish.Events.EventCompetitor do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "event_competitors"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [
        :competitor_id,
        :event_id,
        :session,
        :competitor_number,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :is_active,
        :metadata
      ],
      update: [
        :session,
        :competitor_number,
        :display_name,
        :team_name,
        :level,
        :age_group,
        :is_active,
        :metadata
      ]
    ]
  end

  attributes do
    attribute :id, :string do
      primary_key? true
      allow_nil? false
      default &PhotoFinish.Id.event_competitor_id/0
      writable? false
    end

    attribute :session, :string do
      public? true
      description "Session identifier, e.g. '3A', '11B'"
    end

    attribute :competitor_number, :string do
      allow_nil? false
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
    belongs_to :competitor, PhotoFinish.Events.Competitor do
      public? true
      attribute_type :string
    end

    belongs_to :event, PhotoFinish.Events.Event do
      public? true
      attribute_type :string
    end

    has_many :photos, PhotoFinish.Photos.Photo do
      public? true
    end
  end

  identities do
    identity :unique_event_competitor_number, [:event_id, :competitor_number]
  end
end
