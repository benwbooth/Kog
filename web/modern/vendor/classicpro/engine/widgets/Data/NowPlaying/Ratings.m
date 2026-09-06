#include <lib/std.mi>

Function hoverTo(int rating);
Function setRating(int rating);
Function refreshRating();
Function String convertToText(int rating);

Global Group XUIGroup;
Global Text nostars, normalstars, hoverstarts;
Global Button rate1, rate2, rate3, rate4, rate5;

System.onScriptLoaded(){
	XUIGroup = getScriptGroup();
	nostars = XUIGroup.findObject("rate.line.1");
	normalstars = XUIGroup.findObject("rate.line.2");
	hoverstarts = XUIGroup.findObject("rate.line.3");
	rate1 = XUIGroup.findObject("rate.1");
	rate2 = XUIGroup.findObject("rate.2");
	rate3 = XUIGroup.findObject("rate.3");
	rate4 = XUIGroup.findObject("rate.4");
	rate5 = XUIGroup.findObject("rate.5");
	
	refreshRating();
}

rate1.onLeftClick(){
	if(getCurrentTrackRating()==1) setRating(0);
	else setRating(1);
}
rate2.onLeftClick(){
	setRating(2);
}
rate3.onLeftClick(){
	setRating(3);
}
rate4.onLeftClick(){
	setRating(4);
}
rate5.onLeftClick(){
	setRating(5);
}

String convertToText(int rating){
	String output = "";
	for(int i = 0; i < rating; i++){
		output+="*";
	}
	return output;
}

//Hover of mouse
hoverTo(int rating){
	hoverstarts.setText(convertToText(rating));
}

rate1.onEnterArea(){
	hoverTo(1);
}
rate1.onLeaveArea(){
	hoverTo(0);
}
rate2.onEnterArea(){
	hoverTo(2);
}
rate2.onLeaveArea(){
	hoverTo(0);
}
rate3.onEnterArea(){
	hoverTo(3);
}
rate3.onLeaveArea(){
	hoverTo(0);
}
rate4.onEnterArea(){
	hoverTo(4);
}
rate4.onLeaveArea(){
	hoverTo(0);
}
rate5.onEnterArea(){
	hoverTo(5);
}
rate5.onLeaveArea(){
	hoverTo(0);
}

setRating(int rating){
	setCurrentTrackRating(rating);
	normalstars.setText(convertToText(rating));
}

refreshRating(){
		normalstars.setText(convertToText(getCurrentTrackRating()));
}

System.onCurrentTrackRated(int rating){
	refreshRating();
}

System.onTitleChange(String newtitle){
	refreshRating();
}