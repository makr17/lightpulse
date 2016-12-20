# lightpulse
pulse RGB pixels with selections from one or more light temp ranges 

* loop with specified sleep interval (sleep)
* at any point in time, a null pixel has a random chance to light up (threshold)
* pixel picks color range from one or more temp ranges (temps)
* pixel starts with a random intensity below configured maximum (max_intensity)
* the longer a pixel stays lit, the higher the probability it will decay (decay)
* program runs for configured number of minutes (runfor)

extends whitepulse, which only did white pixels in range from warm to cold

uses [houselights](https://github.com/makr17/houselights) for a lot of boilerplate
