defmodule PhotoFinish.Events do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Events.Event
    resource PhotoFinish.Events.Competitor
    resource PhotoFinish.Events.EventCompetitor
  end
end
