$(document).ready(function(){
    $.get("https://anonfeed.herokuapp.com/feed", function(data) {
        let feed = "";
        data = data.slice(0, 10);
        $.each(data, function() {
            feed += "<div class=\"post\"><strong>" + this.author_handle + ": </strong>" + this.summary + "</div>";
        });
        $("#feed").html(feed);
    });
});

$(function() {
    $('form.postmaker').on('submit', function(e) {
      e.preventDefault();

      var json = {};
      form = $(this)[0];
      $.each(form, function() {
          if (this.id) {
              json[this.id] = this.value;
          }
      });
      json.uuid = "ed8729be-f33e-4395-b1c0-f5673668f89e"; // dummy
      json.contents = "";
      console.log(json);

      $.ajax({
        type: "POST",
        url: "https://anonfeed.herokuapp.com/post",
        data: JSON.stringify(json),
        contentType: 'application/json; charset=utf-8',
        success: function(msg) {
            location.reload();
        },
        dataType: "json"
      });
    });
});
