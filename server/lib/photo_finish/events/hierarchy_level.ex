defmodule PhotoFinish.Events.HierarchyLevel do
  use Ash.Resource,
    otp_app: :photo_finish,
    domain: PhotoFinish.Events,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "hierarchy_levels"
    repo PhotoFinish.Repo
  end

  actions do
    defaults [
      :read,
      :destroy,
      create: [:level_number, :level_name, :level_name_plural, :is_required, :allow_photos, :event_id],
      update: [:level_number, :level_name, :level_name_plural, :is_required, :allow_photos, :event_id]
    ]
  end

  attributes do
    uuid_primary_key :id

    attribute :level_number, :integer do
      allow_nil? false
      public? true
    end

    attribute :level_name, :string do
      allow_nil? false
      public? true
    end

    attribute :level_name_plural, :string do
      public? true
    end

    attribute :is_required, :boolean do
      public? true
      default true
    end

    attribute :allow_photos, :boolean do
      public? true
      default false
    end

    timestamps()
  end

  relationships do
    belongs_to :event, PhotoFinish.Events.Event do
      public? true
    end
  end
end