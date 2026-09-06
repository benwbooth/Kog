#include <lib/std.mi>
#include <lib/winampconfig.mi>

Function setEqBands(String tokenString);

Global Group myGroup, eq_content, eq_holder;
Global Layer eqBand;

//Xfade
Global Button xfadeLess, xfadeMore, set1, set2, set3;
Global Slider xFadeRead;
Global Text xFadeTime;


System.onScriptLoaded() {
	WinampConfigGroup eqwcg = WinampConfig.getGroup("{72409F84-BAF1-4448-8211-D84A30A1591A}");
	int freqmode = eqwcg.getInt("frequencies"); // returns 0 for classical winamp levels, 1 for ISO levels

	myGroup = getScriptGroup();
	eq_content = myGroup.findObject("equalizer.content");
	eq_holder = myGroup.findObject("equalizer.content.holder");
	
	eqBand = myGroup.findObject("equalizer.freq");
	xfadeLess = myGroup.findObject("CrossfadeDecrease");
	xfadeMore = myGroup.findObject("CrossfadeIncrease");
	xFadeRead = myGroup.findObject("sCrossfade");
	xFadeTime = myGroup.findObject("CFDisplay");
	set1 = myGroup.findObject("eq.set.top");
	set2 = myGroup.findObject("eq.set.cen");
	set3 = myGroup.findObject("eq.set.bot");

	xFadeTime.setText(integerToString(xFadeRead.getPosition()));

	System.onEqFreqChanged(freqmode);
}

set1.onLeftClick(){
	setEqBands("127;127;127;127;127;127;127;127;127;127");
}
set2.onLeftClick(){
	setEqBands("0;0;0;0;0;0;0;0;0;0");
}
set3.onLeftClick(){
	setEqBands("-127;-127;-127;-127;-127;-127;-127;-127;-127;-127");
}
setEqBands(String tokenString){
	for(int i=0;i<10;i++){
		System.setEqBand(i, stringToInteger(getToken(tokenString, ";", i)));
	}
}

xfadeLess.onLeftClick(){
	xFadeRead.setPosition(xFadeRead.getPosition()-1);
}
xfadeMore.onLeftClick(){
	xFadeRead.setPosition(xFadeRead.getPosition()+1);
}
xFadeRead.onSetPosition(int newpos){
	xFadeTime.setText(integerToString(newpos));
}

myGroup.onSetVisible(boolean onOff){
	if(onOff) myGroup.onResize(0, 0, myGroup.getWidth(), myGroup.getHeight());
}

myGroup.onResize(int x, int y, int w, int h){
	if(!myGroup.isVisible()) return; //Don'r resize when EQ isn't showing
	if(w<281){
		eq_content.setXmlParam("x", "-57");
		eq_content.setXmlParam("w", "281");
		eq_holder.setXmlParam("w", "224");
		x=w/2-224/2;
	}
	else if(w<377){
		eq_content.setXmlParam("x", "0");
		eq_content.setXmlParam("w", "281");
		eq_holder.setXmlParam("w", "281");
		x=w/2-281/2;
	}
	else{
		eq_content.setXmlParam("x", "0");
		eq_content.setXmlParam("w", "377");
		eq_holder.setXmlParam("w", "377");
		x=w/2-377/2;
	}
	
	y=(h-19)/2-99/2;
	if(y<1) y=1;

	eq_holder.setXmlParam("x", integerToString(x));
	eq_holder.setXmlParam("y", integerToString(y));
}

System.onEqFreqChanged (boolean isoonoff){
	if (isoonoff==1){
		eqBand.setXmlParam("image", "eq.label.iso");
	}
	else{
		eqBand.setXmlParam("image", "eq.label.winamp");
	}
}