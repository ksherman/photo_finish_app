defmodule PhotoFinish.Events do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Events.Event
    resource PhotoFinish.Events.HierarchyLevel
    resource PhotoFinish.Events.HierarchyNode
    resource PhotoFinish.Events.Competitor
  end
end
