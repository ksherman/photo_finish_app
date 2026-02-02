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
        :first_name,
        :last_name,
        :external_id,
        :email,
        :phone,
        :metadata
      ],
      update: [
        :first_name,
        :last_name,
        :external_id,
        :email,
        :phone,
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

    attribute :first_name, :string do
      allow_nil? false
      public? true
    end

    attribute :last_name, :string do
      public? true
    end

    attribute :external_id, :string do
      public? true
    end

    attribute :email, :string do
      public? true
    end

    attribute :phone, :string do
      public? true
    end

    attribute :metadata, :map do
      public? true
    end

    timestamps()
  end

  relationships do
    has_many :event_competitors, PhotoFinish.Events.EventCompetitor do
      public? true
    end
  end
end
